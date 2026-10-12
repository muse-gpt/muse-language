//! Semantic validation and render-plan construction for Muse scripts.
use muse_parser::{BlockContent, BlockKind, Declaration, VisualDecl, parse};
use muse_types::{Diagnostic, RenderSettings, Span};
use serde::Serialize;
use std::collections::BTreeSet;

mod schema;
pub use schema::{
    CompilationDocument, IrAct, IrAsset, IrOrchestrationRule, IrShot, compilation_schema,
};

#[derive(Debug, Clone, Serialize)]
pub struct Act {
    pub name: String,
    pub start_bar: u32,
    pub end_bar: u32,
}
/// Timed cut in the render plan (`shot id : bars scene name`).
#[derive(Debug, Clone, Serialize)]
pub struct Shot {
    pub id: String,
    pub start_bar: u32,
    pub end_bar: u32,
    pub scene: String,
    pub target: Option<String>,
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
    /// Optional scene tag from `visual for act scene name { ... }`.
    pub scene: Option<String>,
    pub background_color: Option<String>,
    /// Image URIs declared as `image "..."` inside the visual block (project-relative).
    pub images: Vec<String>,
    /// Optional full-frame vignette strength in `[0, 1]`.
    pub vignette: Option<f32>,
    /// Optional film grain strength in `[0, 1]`.
    pub grain: Option<f32>,
    /// Optional soft bloom strength in `[0, 1]`.
    pub bloom: Option<f32>,
    /// Built-in palette id (`P2`) or project-relative LUT PNG path.
    pub lut: Option<String>,
    /// LUT mix amount in `[0, 1]` (ignored when `lut` is absent).
    pub lut_mix: Option<f32>,
}
#[derive(Debug, Clone, Serialize)]
pub struct RenderPlan {
    pub settings: RenderSettings,
    pub duration_seconds: f64,
    pub acts: Vec<Act>,
    pub shots: Vec<Shot>,
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
    let mut shots = Vec::new();
    let mut shot_ids = BTreeSet::new();
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
                    if let BlockContent::Timeline {
                        acts: declarations,
                        shots: shot_decls,
                    } = &block.content
                    {
                        for act in declarations {
                            if act.start_bar < act.end_bar {
                                acts.push(Act {
                                    name: act.name.clone(),
                                    start_bar: act.start_bar,
                                    end_bar: act.end_bar,
                                });
                            } else {
                                diagnostics.push(Diagnostic::error(
                                    "act range must be increasing integers",
                                    act.span,
                                ));
                            }
                        }
                        for shot in shot_decls {
                            if shot.start_bar >= shot.end_bar {
                                diagnostics.push(Diagnostic::error(
                                    "shot range must be increasing integers",
                                    shot.span,
                                ));
                                continue;
                            }
                            if !shot_ids.insert(shot.id.clone()) {
                                diagnostics.push(Diagnostic::error(
                                    format!("duplicate shot id `{}`", shot.id),
                                    shot.span,
                                ));
                                continue;
                            }
                            if let Some(target) = &shot.target
                                && !acts.iter().any(|act| act.name == *target)
                            {
                                diagnostics.push(Diagnostic::error(
                                    format!("unknown shot target `{target}`"),
                                    shot.span,
                                ));
                            }
                            shots.push(Shot {
                                id: shot.id.clone(),
                                start_bar: shot.start_bar,
                                end_bar: shot.end_bar,
                                scene: shot.scene.clone(),
                                target: shot.target.clone(),
                            });
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
                    let mut bloom = None;
                    let mut lut = None;
                    let mut lut_mix = None;
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
                                VisualDecl::Bloom { strength, span } => {
                                    match parse_unit_strength(strength) {
                                        Ok(value) => bloom = Some(value),
                                        Err(message) => diagnostics
                                            .push(Diagnostic::error(format!("bloom {message}"), *span)),
                                    }
                                }
                                VisualDecl::Lut { name, mix, span } => {
                                    let name = name.trim().trim_matches('"').to_owned();
                                    if name.is_empty() {
                                        diagnostics.push(Diagnostic::error(
                                            "lut name must be non-empty",
                                            *span,
                                        ));
                                    } else {
                                        lut = Some(name);
                                    }
                                    match parse_unit_strength(mix) {
                                        Ok(value) => lut_mix = Some(value),
                                        Err(message) => diagnostics
                                            .push(Diagnostic::error(format!("lut mix {message}"), *span)),
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
                        scene: block.scene.clone(),
                        background_color,
                        images,
                        vignette,
                        grain,
                        bloom,
                        lut,
                        lut_mix,
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
        shots,
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

impl RenderPlan {
    /// Highest act/shot end bar used for normalized time mapping.
    pub fn end_bar(&self) -> u32 {
        self.acts
            .iter()
            .map(|act| act.end_bar)
            .chain(self.shots.iter().map(|shot| shot.end_bar))
            .max()
            .unwrap_or(0)
    }

    /// Active shot at normalized time `t` in `[0, 1]` across [`Self::end_bar`].
    pub fn shot_at_normalized(&self, t: f32) -> Option<&Shot> {
        if self.shots.is_empty() {
            return None;
        }
        let end = self.end_bar();
        if end == 0 {
            return None;
        }
        let bar = ((t.clamp(0.0, 1.0) as f64) * f64::from(end)).floor() as u32;
        let bar = bar.min(end.saturating_sub(1));
        self.shots
            .iter()
            .find(|shot| shot.start_bar <= bar && bar < shot.end_bar)
    }

    /// Base visuals (no scene tag) plus overlays matching the active shot scene.
    pub fn visuals_at_normalized(&self, t: f32) -> Vec<&VisualLayer> {
        let mut layers: Vec<&VisualLayer> = self
            .visuals
            .iter()
            .filter(|layer| layer.scene.is_none())
            .collect();
        if let Some(shot) = self.shot_at_normalized(t) {
            layers.extend(
                self.visuals
                    .iter()
                    .filter(|layer| layer.scene.as_deref() == Some(shot.scene.as_str())),
            );
        }
        if layers.is_empty() {
            self.visuals.iter().collect()
        } else {
            layers
        }
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

    #[test]
    fn captures_visual_bloom() {
        let c = compile(
            r##"timeline { act main : 0..4 } visual for main { background color "#0b1524" bloom 0.3 }"##,
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid visual");
        assert_eq!(plan.visuals[0].bloom, Some(0.3));
    }

    #[test]
    fn captures_visual_lut() {
        let c = compile(
            r##"timeline { act main : 0..4 } visual for main { background color "#0b1524" lut P2 0.25 }"##,
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid visual");
        assert_eq!(plan.visuals[0].lut.as_deref(), Some("P2"));
        assert_eq!(plan.visuals[0].lut_mix, Some(0.25));
    }

    #[test]
    fn captures_timeline_shots() {
        let c = compile(
            "timeline { act main : 0..16 shot s01 : 0..4 scene ember shot s02 : 4..8 scene illust for main }",
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid shots");
        assert_eq!(plan.shots.len(), 2);
        assert_eq!(plan.shots[0].id, "s01");
        assert_eq!(plan.shots[0].scene, "ember");
        assert_eq!((plan.shots[0].start_bar, plan.shots[0].end_bar), (0, 4));
        assert_eq!(plan.shots[1].target.as_deref(), Some("main"));
    }

    #[test]
    fn rejects_duplicate_shot_ids() {
        let c = compile(
            "timeline { act main : 0..8 shot s01 : 0..2 scene a shot s01 : 2..4 scene b }",
            RenderSettings::default(),
        );
        assert!(!c.is_success());
    }

    #[test]
    fn captures_visual_scene_tag() {
        let c = compile(
            r##"timeline { act main : 0..8 shot s01 : 0..4 scene ember for main } visual for main scene ember { background color "#1a0c08" }"##,
            RenderSettings::default(),
        );
        let plan = c.plan.expect("valid visual scene");
        assert_eq!(plan.visuals[0].scene.as_deref(), Some("ember"));
        assert_eq!(plan.shot_at_normalized(0.0).map(|s| s.id.as_str()), Some("s01"));
        assert_eq!(
            plan.visuals_at_normalized(0.0)
                .iter()
                .filter_map(|v| v.scene.as_deref())
                .collect::<Vec<_>>(),
            ["ember"]
        );
    }
}
