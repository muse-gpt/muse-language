//! wgpu frame compositor: clear + cover-blit image layers → RGBA8 readback.

use anyhow::{Context, Result, bail};
use image::RgbaImage;
use muse_ir::VisualLayer;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct GpuComposeOptions {
    pub width: u32,
    pub height: u32,
    pub asset_root: PathBuf,
}

pub struct GpuCompositor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl GpuCompositor {
    pub fn new() -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .context("GPU compositor: request_adapter failed")?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("muse-gpu-device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
        }))
        .context("GPU compositor: request_device failed")?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("muse-cover-blit"),
            source: wgpu::ShaderSource::Wgsl(COVER_BLIT_WGSL.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("muse-blit-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    // Cover UV remap is applied in the vertex stage.
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("muse-blit-pl"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("muse-cover-blit-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("muse-linear-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Ok(Self {
            device,
            queue,
            pipeline,
            sampler,
            bind_group_layout,
        })
    }

    pub fn compose_layers(
        &self,
        layers: &[VisualLayer],
        options: &GpuComposeOptions,
    ) -> Result<RgbaImage> {
        if options.width == 0 || options.height == 0 {
            bail!("GPU compose size must be non-zero");
        }

        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("muse-frame-target"),
            size: wgpu::Extent3d {
                width: options.width,
                height: options.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

        let mut clear = wgpu::Color {
            r: 11.0 / 255.0,
            g: 21.0 / 255.0,
            b: 36.0 / 255.0,
            a: 1.0,
        };
        let mut image_draws: Vec<(RgbaImage, CoverUv)> = Vec::new();

        for layer in layers {
            if let Some(hex) = layer.background_color.as_deref() {
                clear = parse_hex_color(hex).unwrap_or(clear);
            }
            for uri in &layer.images {
                let path = resolve_asset(uri, &options.asset_root);
                let img = image::open(&path)
                    .with_context(|| {
                        format!(
                            "GPU compose open image `{}` ({})",
                            uri,
                            path.display()
                        )
                    })?
                    .to_rgba8();
                let uv = cover_uv(img.width(), img.height(), options.width, options.height);
                image_draws.push((img, uv));
            }
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("muse-compose-encoder"),
            });

        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("muse-clear-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        let mut upload_keep: Vec<(wgpu::Texture, wgpu::Buffer, wgpu::BindGroup)> = Vec::new();

        for (img, uv) in &image_draws {
            let (tex, uniform_buf, bind_group) = self.upload_image(img, *uv)?;
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("muse-blit-pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                pass.draw(0..6, 0..1);
            }
            upload_keep.push((tex, uniform_buf, bind_group));
        }

        let padded_bytes_per_row = align_256(options.width * 4);
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("muse-frame-readback"),
            size: u64::from(padded_bytes_per_row) * u64::from(options.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &output_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(options.height),
                },
            },
            wgpu::Extent3d {
                width: options.width,
                height: options.height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(Some(encoder.finish()));
        drop(upload_keep);

        let slice = output_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait())
            .context("GPU readback poll failed")?;
        rx.recv()
            .context("GPU readback channel closed")?
            .context("GPU map_async failed")?;

        let data = slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((options.width * options.height * 4) as usize);
        let row_bytes = (options.width * 4) as usize;
        for y in 0..options.height as usize {
            let start = y * padded_bytes_per_row as usize;
            pixels.extend_from_slice(&data[start..start + row_bytes]);
        }
        drop(data);
        output_buffer.unmap();

        RgbaImage::from_raw(options.width, options.height, pixels)
            .context("GPU compose invalid RGBA buffer")
    }

    fn upload_image(
        &self,
        img: &RgbaImage,
        uv: CoverUv,
    ) -> Result<(wgpu::Texture, wgpu::Buffer, wgpu::BindGroup)> {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("muse-layer-texture"),
            size: wgpu::Extent3d {
                width: img.width(),
                height: img.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            img.as_raw(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(img.width() * 4),
                rows_per_image: Some(img.height()),
            },
            wgpu::Extent3d {
                width: img.width(),
                height: img.height(),
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let uniforms = CoverUniforms {
            uv_min: [uv.u0, uv.v0],
            uv_max: [uv.u1, uv.v1],
            _pad: [0.0; 4],
        };
        let uniform_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("muse-cover-uv"),
            size: std::mem::size_of::<CoverUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue
            .write_buffer(&uniform_buf, 0, bytemuck::bytes_of(&uniforms));

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("muse-blit-bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform_buf.as_entire_binding(),
                },
            ],
        });

        Ok((texture, uniform_buf, bind_group))
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CoverUniforms {
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    /// WGSL uniform structs must be 16-byte aligned / sized.
    _pad: [f32; 4],
}

#[derive(Clone, Copy)]
struct CoverUv {
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
}

fn cover_uv(src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> CoverUv {
    let src_aspect = src_w as f32 / src_h as f32;
    let dst_aspect = dst_w as f32 / dst_h as f32;
    if src_aspect > dst_aspect {
        let visible = src_h as f32 * dst_aspect;
        let crop = (src_w as f32 - visible) * 0.5;
        CoverUv {
            u0: crop / src_w as f32,
            v0: 0.0,
            u1: 1.0 - crop / src_w as f32,
            v1: 1.0,
        }
    } else {
        let visible = src_w as f32 / dst_aspect;
        let crop = (src_h as f32 - visible) * 0.5;
        CoverUv {
            u0: 0.0,
            v0: crop / src_h as f32,
            u1: 1.0,
            v1: 1.0 - crop / src_h as f32,
        }
    }
}

fn resolve_asset(uri: &str, root: &Path) -> PathBuf {
    let path = Path::new(uri);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn align_256(value: u32) -> u32 {
    (value + 255) & !255
}

fn parse_hex_color(input: &str) -> Option<wgpu::Color> {
    let hex = input.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f64 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f64 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f64 / 255.0;
    Some(wgpu::Color { r, g, b, a: 1.0 })
}

const COVER_BLIT_WGSL: &str = r#"
struct CoverUv {
    uv_min: vec2<f32>,
    uv_max: vec2<f32>,
    _pad: vec4<f32>,
};

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;
@group(0) @binding(2) var<uniform> cover: CoverUv;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
    );
    var uvs = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, 0.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(positions[idx], 0.0, 1.0);
    let t = uvs[idx];
    out.uv = mix(cover.uv_min, cover.uv_max, t);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(src_tex, src_samp, in.uv);
}
"#;
