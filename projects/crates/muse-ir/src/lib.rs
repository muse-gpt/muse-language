//! Semantic validation and render-plan construction for Muse scripts.
use muse_parser::{BlockContent, BlockKind, Declaration, VisualDecl, parse};
use muse_types::{Diagnostic, RenderSettings, Span};
use serde::Serialize;
use std::collections::BTreeSet;

mod schema;
pub use schema::{CompilationDocument, IrAct, IrAsset, IrOrchestrationRule, compilation_schema};

#[derive(Debug, Clone, Serialize)]
pub struct Act {
    pub name: String,
    pub start_bar: u32,
    pub end_bar: u32,
}
#[derive(Debug, Clone, Serialize)]
pub struct Asset {
    pub name: String,
    pub uri: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct OrchestrationRule {
    pub target: String,
    pub mappings: Vec<String>,
    pub hard_constraints: Vec<String>,
    pub soft_preferences: Vec<String>,
    pub style_references: Vec<String>,
    pub transition_strategy: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct VisualLayer {
    pub target: Option<String>,
    pub background_color: Option<String>,
    /// Image URIs declared as `image "..."` inside the visual block (project-relative).
    pub images: Vec<String>,
    /// Optional full-frame vignette strength in `[0, 1]`.
    pub vignette: Option<f32>,
    /// Optional film grain strength in `[0, 1]`.
    pub grain: Option<f32>,
}
#[derive(Debug, Clone, Serialize)]
pub struct RenderPlan {
    pub settings: RenderSettings,
    pub duration_seconds: f64,
    pub acts: Vec<Act>,
    pub assets: Vec<Asset>,
    pub visual_layers: usize,
    pub visuals: Vec<VisualLayer>,
    pub transitions: usize,
    pub intents: Vec<String>,
    pub orchestration: Vec<OrchestrationRule>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Compilation {
    pub plan: Option<RenderPlan>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Compilation {
    pub fn is_success(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|d| matches!(d.severity, muse_types::Severity::Error))
    }
}

pub fn compile(source: &str, settings: RenderSettings) -> Compilation {
    let parsed = parse(source);
    let mut diagnostics = parsed.diagnostics;
    let mut acts = Vec::new();
    let mut assets = Vec::new();
    let mut names = BTreeSet::new();
    let mut bpm = 120.0;
    let mut visuals = 0;
    let mut visual_layers = Vec::new();
    let mut transitions = 0;
    let mut intents = Vec::new();
    let mut orchestration = Vec::new();
    for declaration in parsed.script.declarations {
        match declaration {
            Declaration::Asset { name, uri, span } => {
                if !names.insert(name.clone()) {
                    diagnostics.push(Diagnostic::error(format!("duplicate asset `{name}`"), span));
                }
                assets.push(Asset { name, uri });
            }
            Declaration::Block(block) => match block.kind {
                BlockKind::Intent => {
                    if let BlockContent::Intent { emotions } = &block.content {
                        intents.extend(emotions.iter().map(|emotion| emotion.emotion.clone()));
                    }
                }
                BlockKind::Timeline => {
                    if let BlockContent::Timeline { acts: declarations } = &block.content {
                        for act in declarations {
                            if act.start_bar < act.end_bar {
                                acts.push(Act {
                                    name: act.name.clone(),
                                    start_bar: act.start_bar,
                                    end_bar: act.end_bar,
                                });
                            } else {
                                diagnostics.push(Diagnostic::error("act range must be increasing integers", act.span));
                            }
                        }
                    }
                }
                BlockKind::Tempo => {
                    if let BlockContent::Tempo { events } = &block.content {
                        for event in events {
                            if event.bpm > 0.0 {
                                bpm = event.bpm;
                            } else {
                                diagnostics.push(Diagnostic::error("BPM must be positive", event.span));
                            }
                        }
                    }
                }
                BlockKind::Music => {
                    if let Some(target) = block.target
                        && !acts.iter().any(|a| a.name == target)
                    {
                        diagnostics.push(Diagnostic::error(
                            format!("unknown music target `{target}`"),
                            block.span,
                        ));
                    }
                    if block.body.contains("avoid-parallel-fifths") && !block.body.contains("harmony") {
                        diagnostics.push(Diagnostic::error(
                            "voice-leading requirement needs a harmony block",
                            block.span,
                        ));
                    }
                }
                BlockKind::Visual => {
                    let mut background_color = None;
                    let mut images = Vec::new();
                    let mut vignette = None;
                    let mut grain = None;
                    if let BlockContent::Visual { elements } = &block.content {
                        visuals += elements.len().max(1);
                        for element in elements {
                            match element {
                                VisualDecl::Background { color, .. } => {
                                    background_color = Some(color.trim().trim_matches('"').to_owned());
                                }
                                VisualDecl::Asset {
                                    kind: muse_parser::VisualAssetKind::Image,
                                    uri,
                                    ..
                                } => {
                                    images.push(uri.trim().trim_matches('"').to_owned());
                                }
                                VisualDecl::Vignette { strength, span } => {
                                    match parse_unit_strength(strength) {
                                        Ok(value) => vignette = Some(value),
                                        Err(message) => diagnostics
                                            .push(Diagnostic::error(format!("vignette {message}"), *span)),
                                    }
                                }
                                VisualDecl::Grain { strength, span } => {
                                    match parse_unit_strength(strength) {
                                        Ok(value) => grain = Some(value),
                                        Err(message) => diagnostics
                                            .push(Diagnostic::error(format!("grain {message}"), *span)),
                                    }
                                }
                                _ => {}
                            }
                        }
                    } else {
                        visuals += 1;
                    }
                    if let Some(target) = &block.target
                        && !acts.iter().any(|a| a.name == *target)
                    {
                        diagnostics.push(Diagnostic::error(
                            format!("unknown visual target `{target}`"),
                            block.span,
                        ));
                    }
                    visual_layers.push(VisualLayer {
                        target: block.target.clone(),
                        background_color,
                        images,
                        vignette,
                        grain,
                    });
                }
                BlockKind::Orchestrate => {
                    let target = block.target.unwrap_or_else(|| "entire_song".to_owned());
                    if target != "entire_song" && !acts.iter().any(|act| act.name == target) {
                        diagnostics.push(Diagnostic::error(
                            format!("unknown orchestration target `{target}`"),
                            block.span,
                        ));
                    }
                    let BlockContent::Orchestrate(declaration) = &block.content else {
                        continue;
                    };
                    let hard_constraints = declaration.hard_constraints.clone();
                    let soft_preferences = declaration.soft_preferences.clone();
                    let style_references = declaration.style_references.clone();
                    let transition_strategy = declaration.transition_strategy.clone();
                    transitions += usize::from(transition_strategy.is_some());
                    orchestration.push(OrchestrationRule {
                        target,
                        mappings: declaration
                            .mappings
                            .iter()
                            .map(|mapping| mapping.intent.clone())
                            .collect(),
                        hard_constraints,
                        soft_preferences,
                        style_references,
                        transition_strategy,
                    });
                }
                BlockKind::Directing => {
                    transitions += block.body.matches("transition at").count();
                }
            },
            _ => {}
        }
    }
    if acts.is_empty() {
        diagnostics.push(Diagnostic::error(
            "a timeline with at least one act is required",
            Span::new(0, 0, 1, 1),
        ));
    }
    let end = acts.iter().map(|a| a.end_bar).max().unwrap_or(0);
    let duration = (end as f64) * 4.0 * 60.0 / bpm;
    let plan = (!diagnostics
        .iter()
        .any(|d| matches!(d.severity, muse_types::Severity::Error)))
    .then_some(RenderPlan {
        settings,
        duration_seconds: duration,
        acts,
        assets,
        visual_layers: visuals,
        visuals: visual_layers,
        transitions,
        intents,
        orchestration,
    });
    Compilation { plan, diagnostics }
}

fn parse_unit_strength(raw: &str) -> Result<f32, &'static str> {
    match raw.trim().parse::<f32>() {
        Ok(value) if (0.0..=1.0).contains(&value) => Ok(value),
        Ok(_) => Err("strength must be between 0 and 1"),
        Err(_) => Err("strength must be a number"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unknown_target() {
        let c = compile("timeline { act a : 0..2 } visual for b {}", RenderSettings::default());
        assert!(!c.is_success())
    }

    #[test]
    fn emits_orchestration_rules() {
        let c = compile(
            "timeline { act verse : 0..4 } orchestrate for verse { map \"loneliness\" to {} hard_constraints: [\"no_glitch\"] }",
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid orchestration");
        assert_eq!(plan.orchestration[0].mappings, ["loneliness"]);
        assert_eq!(plan.orchestration[0].hard_constraints, ["no_glitch"]);
    }

    #[test]
    fn captures_visual_background_color() {
        let c = compile(
            r##"timeline { act main : 0..4 } visual for main { background color "#0b1524" }"##,
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid visual");
        assert_eq!(plan.visuals.len(), 1);
        assert_eq!(plan.visuals[0].background_color.as_deref(), Some("#0b1524"));
    }

    #[test]
    fn captures_visual_image_uris() {
        let c = compile(
            r##"timeline { act main : 0..4 } visual for main { background color "#000000" image "assets/textures/plate.png" }"##,
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid visual");
        assert_eq!(plan.visuals[0].images, ["assets/textures/plate.png"]);
    }

    #[test]
    fn captures_visual_vignette() {
        let c = compile(
            r##"timeline { act main : 0..4 } visual for main { background color "#0b1524" vignette 0.35 }"##,
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid visual");
        assert_eq!(plan.visuals[0].vignette, Some(0.35));
    }

    #[test]
    fn captures_visual_grain() {
        let c = compile(
            r##"timeline { act main : 0..4 } visual for main { background color "#0b1524" grain 0.06 }"##,
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid visual");
        assert_eq!(plan.visuals[0].grain, Some(0.06));
    }
}
