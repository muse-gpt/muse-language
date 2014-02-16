# muse-score

`muse-score` 是 Muse 的纯 Rust 乐谱排版层。

设计约束：

- 不调用外部 CLI；
- 不链接 FFI/C/C++ 乐谱引擎；
- 不要求系统安装 MuseScore、LilyPond、Verovio、ffmpeg、LAME 或 FluidSynth；
- 输入只接受 Muse 内部的 `muse_music::Score`；
- SVG 是布局的规范输出，HTML 通过内嵌 SVG 展示，PDF 使用仓库内的 Rust 矢量写出器；
- 所有输出必须由相同的 `Score` 和相同的确定性布局参数生成。

以后如果增强音符、连线、休止、声部和分页排版，应该继续扩展本 crate 的布局模型和符号绘制，不引入本机依赖作为后门路径。
# muse-score

`muse-score` is Muse's self-contained Rust score layout and drawing crate.

The score layout is backend-neutral: semantic measures are laid out once into
lines, noteheads, rests, glyphs, and text; SVG, HTML, and PDF then consume the
same scene. The crate does not invoke a local notation application, command-line
renderer, FFI bridge, or browser at runtime.

`assets/Bravura.otf` supplies SMuFL music-glyph outlines under the SIL Open Font
License 1.1 (the complete notice is `assets/Bravura-LICENSE.txt`). The renderer
extracts those outlines with pure Rust `ttf-parser` and emits native SVG/PDF
paths. Therefore PDF output does not degrade when a reader lacks a music font.
