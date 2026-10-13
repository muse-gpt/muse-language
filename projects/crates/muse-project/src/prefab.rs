//! Prefab definition load/validate from true VON (docs 19/20).
//!
//! First slice: port + object/component graph identity checks. No scene instantiation yet.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
};

/// Closed component type set for the first prefab slice (doc 20 §4).
pub const KNOWN_COMPONENT_TYPES: &[&str] = &[
    "Transform",
    "LayoutBox",
    "Text",
    "LocalizedTextBinding",
    "LocalizedAssetBinding",
    "LyricPresentation",
    "Framing",
    "ResponsiveBinding",
    "Animation",
    "AudioBinding",
    "TimelineBinding",
];

/// Closed port type names (doc 20 §4).
pub const KNOWN_PORT_TYPES: &[&str] = &[
    "CueId",
    "TextId",
    "AssetRef",
    "ObjectRef",
    "Locale",
    "f32",
    "bool",
    "Color",
    "Range",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefabDefinition {
    pub id: String,
    /// `prefab` or `prefab_variant`.
    pub kind: String,
    /// Required when `kind` is `prefab_variant`.
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub ports: Vec<PrefabPort>,
    #[serde(default)]
    pub objects: Vec<PrefabObject>,
    #[serde(default)]
    pub nested: Vec<PrefabNestedInstance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefabPort {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: String,
    /// Optional default literal (string form for VON-friendly round-trip).
    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefabObject {
    pub id: String,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub components: Vec<PrefabComponent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefabComponent {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefabNestedInstance {
    pub id: String,
    pub prefab: String,
}

/// Load a prefab definition from a true VON file (`oak-von`).
pub fn load_prefab_von(path: &Path) -> Result<PrefabDefinition> {
    let raw = fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let def: PrefabDefinition = oak_von::from_str(&raw)
        .map_err(|err| anyhow::anyhow!("invalid VON prefab {}: {err}", path.display()))?;
    validate_prefab(&def)?;
    Ok(def)
}

/// Structural validation for a prefab definition (no filesystem resolution of nested refs).
pub fn validate_prefab(def: &PrefabDefinition) -> Result<()> {
    if def.id.trim().is_empty() {
        bail!("prefab id is required");
    }
    match def.kind.as_str() {
        "prefab" => {
            if def.base.as_ref().is_some_and(|b| !b.trim().is_empty()) {
                bail!(
                    "prefab `{}` kind `prefab` must not set base (use prefab_variant)",
                    def.id
                );
            }
        }
        "prefab_variant" => {
            if def.base.as_ref().map(|b| b.trim()).is_none_or(|b| b.is_empty()) {
                bail!(
                    "prefab_variant `{}` requires non-empty base",
                    def.id
                );
            }
        }
        other => bail!(
            "prefab `{}` unknown kind `{other}` (expected prefab|prefab_variant)",
            def.id
        ),
    }

    let mut port_ids = BTreeSet::new();
    for port in &def.ports {
        if port.id.trim().is_empty() {
            bail!("prefab `{}` has a port with empty id", def.id);
        }
        if !port_ids.insert(port.id.clone()) {
            bail!("prefab `{}` duplicate port id `{}`", def.id, port.id);
        }
        if !KNOWN_PORT_TYPES.iter().any(|t| *t == port.ty.as_str()) {
            bail!(
                "prefab `{}` port `{}` unknown type `{}`",
                def.id,
                port.id,
                port.ty
            );
        }
    }

    let mut object_ids = BTreeSet::new();
    for obj in &def.objects {
        if obj.id.trim().is_empty() {
            bail!("prefab `{}` has an object with empty id", def.id);
        }
        if !object_ids.insert(obj.id.clone()) {
            bail!("prefab `{}` duplicate object id `{}`", def.id, obj.id);
        }
        let mut component_ids = BTreeSet::new();
        for comp in &obj.components {
            if comp.id.trim().is_empty() {
                bail!(
                    "prefab `{}` object `{}` has a component with empty id",
                    def.id,
                    obj.id
                );
            }
            if !component_ids.insert(comp.id.clone()) {
                bail!(
                    "prefab `{}` object `{}` duplicate component id `{}`",
                    def.id,
                    obj.id,
                    comp.id
                );
            }
            if !KNOWN_COMPONENT_TYPES.iter().any(|t| *t == comp.ty.as_str()) {
                bail!(
                    "prefab `{}` object `{}` component `{}` unknown type `{}`",
                    def.id,
                    obj.id,
                    comp.id,
                    comp.ty
                );
            }
        }
    }

    for obj in &def.objects {
        if let Some(parent) = obj.parent.as_ref().map(|p| p.trim()).filter(|p| !p.is_empty()) {
            if !object_ids.contains(parent) {
                bail!(
                    "prefab `{}` object `{}` parent `{}` not found",
                    def.id,
                    obj.id,
                    parent
                );
            }
            if parent == obj.id {
                bail!(
                    "prefab `{}` object `{}` cannot parent itself",
                    def.id,
                    obj.id
                );
            }
        }
    }

    let mut nested_ids = BTreeSet::new();
    for nested in &def.nested {
        if nested.id.trim().is_empty() {
            bail!("prefab `{}` has a nested instance with empty id", def.id);
        }
        if !nested_ids.insert(nested.id.clone()) {
            bail!(
                "prefab `{}` duplicate nested instance id `{}`",
                def.id,
                nested.id
            );
        }
        if nested.prefab.trim().is_empty() {
            bail!(
                "prefab `{}` nested `{}` missing prefab ref",
                def.id,
                nested.id
            );
        }
    }

    Ok(())
}

/// Discover `assets/prefabs/**/*.von` under a project root (non-recursive missing dir → empty).
pub fn discover_prefab_paths(project: &Path) -> Result<Vec<PathBuf>> {
    let root = project.join("assets").join("prefabs");
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    walk_von(&root, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk_von(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_von(&path, out)?;
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("von"))
        {
            out.push(path);
        }
    }
    Ok(())
}

/// Load and validate all prefabs under the project; also checks nested refs resolve by id.
pub fn check_project_prefabs(project: &Path) -> Result<Vec<PrefabDefinition>> {
    let paths = discover_prefab_paths(project)?;
    let mut by_id: HashMap<String, PathBuf> = HashMap::new();
    let mut defs = Vec::with_capacity(paths.len());
    for path in &paths {
        let def = load_prefab_von(path)?;
        if let Some(prev) = by_id.insert(def.id.clone(), path.clone()) {
            bail!(
                "duplicate prefab id `{}` in {} and {}",
                def.id,
                prev.display(),
                path.display()
            );
        }
        defs.push(def);
    }
    for def in &defs {
        for nested in &def.nested {
            if !by_id.contains_key(nested.prefab.as_str()) {
                bail!(
                    "prefab `{}` nested `{}` references unknown prefab `{}`",
                    def.id,
                    nested.id,
                    nested.prefab
                );
            }
        }
        if def.kind == "prefab_variant" {
            if let Some(base) = def.base.as_deref() {
                if !by_id.contains_key(base) {
                    bail!(
                        "prefab_variant `{}` base `{}` not found under assets/prefabs",
                        def.id,
                        base
                    );
                }
            }
        }
    }
    Ok(defs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn loads_and_validates_lyric_card_von() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = env::temp_dir().join(format!("muse-prefab-{stamp}"));
        let dir = root.join("assets/prefabs");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("LyricCard.von");
        fs::write(
            &path,
            r#"{
  id: "LyricCard"
  kind: "prefab"
  ports: [
    { id: "cue", type: "CueId" }
    { id: "intensity", type: "f32", default: "1.0" }
  ]
  objects: [
    {
      id: "root"
      components: [ { id: "layout", type: "LayoutBox" } ]
    }
    {
      id: "caption"
      parent: "root"
      components: [
        { id: "text", type: "Text" }
        { id: "binding", type: "LocalizedTextBinding" }
        { id: "lyric", type: "LyricPresentation" }
      ]
    }
  ]
}
"#,
        )
        .unwrap();
        let def = load_prefab_von(&path).unwrap();
        assert_eq!(def.id, "LyricCard");
        assert_eq!(def.ports.len(), 2);
        assert_eq!(def.objects[1].parent.as_deref(), Some("root"));
        let checked = check_project_prefabs(&root).unwrap();
        assert_eq!(checked.len(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_toml_shaped_prefab_von() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!("muse-prefab-toml-{stamp}.von"));
        fs::write(
            &path,
            r#"[prefab]
id = "Bad"
kind = "prefab"
"#,
        )
        .unwrap();
        let err = load_prefab_von(&path).unwrap_err().to_string();
        assert!(err.contains("invalid VON"), "{err}");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_unknown_component_type() {
        let err = validate_prefab(&PrefabDefinition {
            id: "X".into(),
            kind: "prefab".into(),
            base: None,
            ports: vec![],
            objects: vec![PrefabObject {
                id: "root".into(),
                parent: None,
                components: vec![PrefabComponent {
                    id: "c".into(),
                    ty: "NotAThing".into(),
                }],
            }],
            nested: vec![],
        })
        .unwrap_err()
        .to_string();
        assert!(err.contains("unknown type"), "{err}");
    }
}
