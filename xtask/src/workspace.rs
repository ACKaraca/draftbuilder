//! Gathers the dependency facts of this workspace from `cargo metadata`, the manifests and the
//! sources, for the pure rule function in [`crate::layers`].

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::layers::{Crate, Dep, DepKind, Facts, Table};

/// A task failure, already worded for the terminal.
pub type Failure = String;

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    workspace_members: Vec<String>,
    workspace_root: PathBuf,
    resolve: Resolve,
}

#[derive(Deserialize)]
struct Package {
    id: String,
    name: String,
    manifest_path: PathBuf,
    #[serde(default)]
    features: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    dependencies: Vec<Declared>,
}

/// A dependency as the manifest declares it.
#[derive(Deserialize)]
struct Declared {
    name: String,
    #[serde(default)]
    optional: bool,
    kind: Option<String>,
}

impl Package {
    /// The names of the optional normal dependencies that the default features leave off.
    fn optional_off(&self) -> Vec<&str> {
        let mut on = Vec::new();
        let mut seen = Vec::new();
        let mut pending = vec!["default"];
        while let Some(feature) = pending.pop() {
            if seen.contains(&feature) {
                continue;
            }
            seen.push(feature);
            for item in self.features.get(feature).into_iter().flatten() {
                if let Some(dep) = item.strip_prefix("dep:") {
                    on.push(dep);
                } else if let Some((dep, _)) = item.split_once('/') {
                    if !dep.ends_with('?') {
                        on.push(dep);
                    }
                } else {
                    pending.push(item);
                }
            }
        }
        self.dependencies
            .iter()
            .filter(|d| d.optional && d.kind.is_none() && !on.contains(&d.name.as_str()))
            .map(|d| d.name.as_str())
            .collect()
    }
}

#[derive(Deserialize)]
struct Resolve {
    nodes: Vec<Node>,
}

#[derive(Deserialize)]
struct Node {
    id: String,
    deps: Vec<NodeDep>,
}

#[derive(Deserialize)]
struct NodeDep {
    pkg: String,
    dep_kinds: Vec<NodeDepKind>,
}

#[derive(Deserialize)]
struct NodeDepKind {
    kind: Option<String>,
}

/// The `cargo` executable that runs this task.
pub fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

/// The workspace root: the parent of this crate's directory.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

/// Reads `xtask/layers.toml`.
pub fn load_table() -> Result<Table, Failure> {
    let path = root().join("xtask").join("layers.toml");
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// A workspace crate as far as the catalog gate needs it.
pub struct Member {
    pub name: String,
    /// The crate directory.
    pub dir: PathBuf,
}

/// The workspace crates, from `cargo metadata`.
pub fn load_members() -> Result<Vec<Member>, Failure> {
    let metadata = run_metadata()?;
    Ok(metadata
        .packages
        .iter()
        .filter(|p| metadata.workspace_members.contains(&p.id))
        .map(|p| Member {
            name: p.name.clone(),
            dir: p
                .manifest_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
        })
        .collect())
}

fn run_metadata() -> Result<Metadata, Failure> {
    let output = Command::new(cargo())
        .args(["metadata", "--format-version", "1"])
        .current_dir(root())
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|e| format!("cargo metadata: {e}"))
}

/// Runs `cargo metadata` and reads manifests and sources of the workspace crates.
pub fn load_facts() -> Result<Facts, Failure> {
    let metadata = run_metadata()?;
    let forbids_unsafe = workspace_forbids_unsafe(&metadata.workspace_root)?;
    let names: BTreeMap<&str, &Package> = metadata
        .packages
        .iter()
        .map(|p| (p.id.as_str(), p))
        .collect();
    let mut facts = Facts::default();
    for node in &metadata.resolve.nodes {
        let Some(package) = names.get(node.id.as_str()) else {
            continue;
        };
        let mut deps = Vec::new();
        let optional_off = package.optional_off();
        for dep in &node.deps {
            let Some(target) = names.get(dep.pkg.as_str()) else {
                continue;
            };
            for kind in &dep.dep_kinds {
                let kind = match kind.kind.as_deref() {
                    Some("dev") => DepKind::Dev,
                    Some("build") => DepKind::Build,
                    _ if optional_off.contains(&target.name.as_str()) => DepKind::Optional,
                    _ => DepKind::Normal,
                };
                deps.push(Dep {
                    name: target.name.clone(),
                    kind,
                });
            }
        }
        if metadata.workspace_members.contains(&node.id) {
            let declares_unsafe = !forbids_unsafe || crate_declares_unsafe(&package.manifest_path)?;
            facts.workspace.push(Crate {
                name: package.name.clone(),
                declares_unsafe,
                deps,
            });
        } else {
            let normal = deps
                .into_iter()
                .filter(|d| d.kind == DepKind::Normal)
                .map(|d| d.name);
            facts
                .external
                .entry(package.name.clone())
                .or_default()
                .extend(normal);
        }
    }
    Ok(facts)
}

fn read_manifest(path: &Path) -> Result<toml::Table, Failure> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    text.parse().map_err(|e| format!("{}: {e}", path.display()))
}

fn workspace_forbids_unsafe(workspace_root: &Path) -> Result<bool, Failure> {
    let manifest = read_manifest(&workspace_root.join("Cargo.toml"))?;
    let level = manifest
        .get("workspace")
        .and_then(|w| w.get("lints"))
        .and_then(|l| l.get("rust"))
        .and_then(|r| r.get("unsafe_code"))
        .and_then(toml::Value::as_str);
    Ok(level == Some("forbid"))
}

/// A crate declares `unsafe` when it does not inherit the workspace lint table or when one of its
/// sources contains an `unsafe` construct.
fn crate_declares_unsafe(manifest_path: &Path) -> Result<bool, Failure> {
    let manifest = read_manifest(manifest_path)?;
    let inherits = manifest
        .get("lints")
        .and_then(|l| l.get("workspace"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    if !inherits {
        return Ok(true);
    }
    let Some(dir) = manifest_path.parent() else {
        return Ok(false);
    };
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let entries = fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in entries {
            let path = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n != "target") {
                    pending.push(path);
                }
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text =
                    fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                if source_declares_unsafe(&text) {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

// shortcut: a textual scan, so an `unsafe` construct quoted inside a string literal counts too;
// replace with a syntax-aware scan if that ever blocks legitimate code.
fn source_declares_unsafe(text: &str) -> bool {
    // The keyword is assembled so that this file does not match its own patterns.
    let keyword = ["un", "safe"].concat();
    text.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .any(|line| {
            line.match_indices(&keyword).any(|(at, _)| {
                let before = line[..at].chars().next_back();
                let after = line[at + keyword.len()..].trim_start();
                !before.is_some_and(|c| c.is_alphanumeric() || c == '_')
                    && ["{", "fn ", "impl ", "impl<", "trait ", "extern "]
                        .iter()
                        .any(|next| after.starts_with(next))
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyword() -> String {
        ["un", "safe"].concat()
    }

    #[test]
    fn unsafe_constructs_are_detected() {
        for tail in [
            "{ work() }",
            "fn f() {}",
            "impl Send for T {}",
            "extern \"C\" {}",
        ] {
            let text = format!("fn outer() {{\n    {} {tail}\n}}\n", keyword());
            assert!(source_declares_unsafe(&text), "{text}");
        }
    }

    #[test]
    fn comments_lint_names_and_identifiers_are_not_unsafe_code() {
        let k = keyword();
        let text = format!(
            "// {k} fn in a comment\n#![forbid({k}_code)]\nfn not_{k} () {{}}\nlet {k}ty = 1;\n"
        );
        assert!(!source_declares_unsafe(&text));
    }

    #[test]
    fn this_workspace_forbids_unsafe_and_declares_none() {
        assert!(workspace_forbids_unsafe(&root()).unwrap());
        assert!(!crate_declares_unsafe(&root().join("xtask").join("Cargo.toml")).unwrap());
    }
}
