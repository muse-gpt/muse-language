//! Stable, machine-readable interchange schema for the semantic IR.
//!
//! These DTOs intentionally do not expose parser AST nodes. They are the
//! versioned contract consumed by CLI `--emit json` and MCP clients.
use crate::{Act, Asset, OrchestrationRule, RenderPlan, Shot, VisualLayer};
use muse_types::RenderSettings;
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct IrAct {
    pub name: String,
    pub start_bar: u32,
    pub end_bar: u32,
}
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct IrShot {
    pub id: String,
    pub start_bar: u32,
    pub end_bar: u32,
    pub scene: String,
    pub target: Option<String>,
    pub transition: String,
    pub transition_bars: f32,
}
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct IrAsset {
    pub name: String,
    pub uri: String,
}
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct IrOrchestrationRule {
    pub target: String,
    pub mappings: Vec<String>,
    pub hard_constraints: Vec<String>,
    pub soft_preferences: Vec<String>,
    pub style_references: Vec<String>,
    pub transition_strategy: Option<String>,
}
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct IrVisualLayer {
    pub target: Option<String>,
    pub scene: Option<String>,
    pub background_color: Option<String>,
    pub images: Vec<String>,
    pub vignette: Option<f32>,
    pub grain: Option<f32>,
    pub bloom: Option<f32>,
    pub lut: Option<String>,
    pub lut_mix: Option<f32>,
    pub pulse: Option<f32>,
    pub stamp: Option<f32>,
    pub stamp_label: Option<String>,
}
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CompilationDocument {
    pub settings: RenderSettings,
    pub duration_seconds: f64,
    pub acts: Vec<IrAct>,
    pub shots: Vec<IrShot>,
    pub assets: Vec<IrAsset>,
    pub visual_layers: usize,
    pub visuals: Vec<IrVisualLayer>,
    pub transitions: usize,
    pub intents: Vec<String>,
    pub orchestration: Vec<IrOrchestrationRule>,
}

impl From<&Act> for IrAct {
    fn from(value: &Act) -> Self {
        Self {
            name: value.name.clone(),
            start_bar: value.start_bar,
            end_bar: value.end_bar,
        }
    }
}
impl From<&Shot> for IrShot {
    fn from(value: &Shot) -> Self {
        Self {
            id: value.id.clone(),
            start_bar: value.start_bar,
            end_bar: value.end_bar,
            scene: value.scene.clone(),
            target: value.target.clone(),
            transition: value.transition.as_str().into(),
            transition_bars: value.transition_bars,
        }
    }
}
impl From<&Asset> for IrAsset {
    fn from(value: &Asset) -> Self {
        Self {
            name: value.name.clone(),
            uri: value.uri.clone(),
        }
    }
}
impl From<&OrchestrationRule> for IrOrchestrationRule {
    fn from(value: &OrchestrationRule) -> Self {
        Self {
            target: value.target.clone(),
            mappings: value.mappings.clone(),
            hard_constraints: value.hard_constraints.clone(),
            soft_preferences: value.soft_preferences.clone(),
            style_references: value.style_references.clone(),
            transition_strategy: value.transition_strategy.clone(),
        }
    }
}
impl From<&VisualLayer> for IrVisualLayer {
    fn from(value: &VisualLayer) -> Self {
        Self {
            target: value.target.clone(),
            scene: value.scene.clone(),
            background_color: value.background_color.clone(),
            images: value.images.clone(),
            vignette: value.vignette,
            grain: value.grain,
            bloom: value.bloom,
            lut: value.lut.clone(),
            lut_mix: value.lut_mix,
            pulse: value.pulse,
            stamp: value.stamp,
            stamp_label: value.stamp_label.clone(),
        }
    }
}
impl From<&RenderPlan> for CompilationDocument {
    fn from(value: &RenderPlan) -> Self {
        Self {
            settings: value.settings.clone(),
            duration_seconds: value.duration_seconds,
            acts: value.acts.iter().map(Into::into).collect(),
            shots: value.shots.iter().map(Into::into).collect(),
            assets: value.assets.iter().map(Into::into).collect(),
            visual_layers: value.visual_layers,
            visuals: value.visuals.iter().map(Into::into).collect(),
            transitions: value.transitions,
            intents: value.intents.clone(),
            orchestration: value.orchestration.iter().map(Into::into).collect(),
        }
    }
}
pub fn compilation_schema() -> schemars::Schema {
    schemars::schema_for!(CompilationDocument)
}
