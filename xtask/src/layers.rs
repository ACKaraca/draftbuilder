//! The layering rules (ADR-0001): a pure function from the layer table and the dependency facts
//! of the workspace to the list of violations.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Deserialize;

/// The highest layer that must stay free of windowing, GPU and UI crates.
const HEADLESS_MAX_LAYER: u8 = 4;

/// `xtask/layers.toml` as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    ui_deny: BTreeSet<String>,
    /// The foundation crates of `arc-core` and the third-party crates a workspace crate may
    /// depend on directly.
    third_party: BTreeSet<String>,
    crates: BTreeMap<String, String>,
    #[serde(default)]
    same_layer: Vec<SameLayerEdge>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SameLayerEdge {
    from: String,
    to: String,
}

/// The class of a workspace crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Layer(u8),
    Ffi(u8),
    App,
    Testkit,
    Tool,
}

impl Class {
    fn parse(text: &str) -> Option<Self> {
        match text {
            "app" => return Some(Self::App),
            "testkit" => return Some(Self::Testkit),
            "tool" => return Some(Self::Tool),
            _ => {}
        }
        let (kind, rest) = text.split_once('(')?;
        let layer = rest.strip_suffix(')')?.parse().ok()?;
        match kind {
            "layer" => Some(Self::Layer(layer)),
            "ffi" => Some(Self::Ffi(layer)),
            _ => None,
        }
    }

    fn layer(self) -> Option<u8> {
        match self {
            Self::Layer(n) | Self::Ffi(n) => Some(n),
            Self::App | Self::Testkit | Self::Tool => None,
        }
    }
}

/// How a dependency is declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepKind {
    Normal,
    /// A normal dependency that is optional and that the crate's default features leave off.
    Optional,
    Dev,
    Build,
}

/// One direct dependency of a workspace crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dep {
    pub name: String,
    pub kind: DepKind,
}

/// What is known about one workspace crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crate {
    pub name: String,
    /// The crate contains `unsafe` code or does not inherit the workspace lint table.
    pub declares_unsafe: bool,
    pub deps: Vec<Dep>,
}

/// The dependency facts of a workspace.
#[derive(Debug, Default)]
pub struct Facts {
    pub workspace: Vec<Crate>,
    /// Normal dependencies of every third-party package, by package name.
    pub external: BTreeMap<String, Vec<String>>,
}

/// The rule a violation breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// The table itself is malformed.
    Table,
    /// A workspace crate has no class in the table.
    Unclassified,
    /// A crate depends on a crate of a higher layer.
    Upward,
    /// A crate depends on a crate of its own layer without a listed edge.
    SameLayer,
    /// A crate of L0-L4 reaches a windowing, GPU or UI crate.
    Ui,
    /// `unsafe` code outside an FFI crate.
    Unsafe,
    /// A dependency on an application, tool or (outside dev-dependencies) test-kit crate.
    NotALibrary,
    /// A third-party dependency that is not in the technology baseline.
    ThirdParty,
}

/// One broken rule, naming the crate and the offending edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub rule: Rule,
    pub krate: String,
    pub detail: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}] {}: {}", self.rule, self.krate, self.detail)
    }
}

/// Evaluates every rule; an empty result means the workspace respects the table.
pub fn check(table: &Table, facts: &Facts) -> Vec<Violation> {
    let mut out = Vec::new();
    let mut classes = BTreeMap::new();
    for (name, text) in &table.crates {
        match Class::parse(text) {
            Some(class) => {
                classes.insert(name.as_str(), class);
            }
            None => out.push(violation(
                Rule::Table,
                name,
                format!("unknown class `{text}`"),
            )),
        }
    }
    let members: BTreeSet<&str> = facts.workspace.iter().map(|c| c.name.as_str()).collect();

    for krate in &facts.workspace {
        let Some(&class) = classes.get(krate.name.as_str()) else {
            let detail = "not listed in xtask/layers.toml".to_owned();
            out.push(violation(Rule::Unclassified, &krate.name, detail));
            continue;
        };
        if krate.declares_unsafe && !matches!(class, Class::Ffi(_)) {
            let detail = "contains `unsafe` code or does not inherit the workspace lints, \
                          and is not an FFI crate"
                .to_owned();
            out.push(violation(Rule::Unsafe, &krate.name, detail));
        }
        for dep in &krate.deps {
            if !members.contains(dep.name.as_str()) {
                if !table.third_party.contains(&dep.name) {
                    let detail = format!("`{}` is not in the technology baseline", dep.name);
                    out.push(violation(Rule::ThirdParty, &krate.name, detail));
                }
            } else if let Some(&target) = classes.get(dep.name.as_str()) {
                out.extend(check_edge(table, krate, class, dep, target));
            }
        }
        if class.layer().is_some_and(|n| n <= HEADLESS_MAX_LAYER) {
            out.extend(reach(&table.ui_deny, facts, krate).map(|path| {
                violation(Rule::Ui, &krate.name, format!("reaches a UI crate: {path}"))
            }));
        }
    }
    out
}

fn violation(rule: Rule, krate: &str, detail: String) -> Violation {
    Violation {
        rule,
        krate: krate.to_owned(),
        detail,
    }
}

fn check_edge(
    table: &Table,
    from: &Crate,
    class: Class,
    dep: &Dep,
    target: Class,
) -> Option<Violation> {
    let edge = format!("{} -> {}", from.name, dep.name);
    match target {
        Class::App | Class::Tool => {
            return Some(violation(Rule::NotALibrary, &from.name, edge));
        }
        Class::Testkit if dep.kind != DepKind::Dev && class != Class::Testkit => {
            return Some(violation(Rule::NotALibrary, &from.name, edge));
        }
        _ => {}
    }
    let (from_layer, to_layer) = (class.layer()?, target.layer()?);
    if to_layer > from_layer {
        let detail = format!("{edge} (L{from_layer} -> L{to_layer})");
        return Some(violation(Rule::Upward, &from.name, detail));
    }
    let listed = table
        .same_layer
        .iter()
        .any(|e| e.from == from.name && e.to == dep.name);
    if to_layer == from_layer && !listed {
        let detail = format!("{edge} (both L{from_layer}, edge not listed)");
        return Some(violation(Rule::SameLayer, &from.name, detail));
    }
    None
}

/// Finds a chain of normal dependencies from `root` to a crate of `deny`.
fn reach(deny: &BTreeSet<String>, facts: &Facts, root: &Crate) -> Option<String> {
    let normal_deps = |name: &str| -> Vec<&str> {
        match facts.workspace.iter().find(|c| c.name == name) {
            Some(krate) => krate
                .deps
                .iter()
                .filter(|d| d.kind == DepKind::Normal)
                .map(|d| d.name.as_str())
                .collect(),
            None => facts
                .external
                .get(name)
                .map(|deps| deps.iter().map(String::as_str).collect())
                .unwrap_or_default(),
        }
    };
    // Depth-first search with an explicit stack; `seen` bounds it to the size of the graph.
    let mut seen = BTreeSet::new();
    let mut stack = vec![(root.name.as_str(), root.name.clone())];
    while let Some((name, path)) = stack.pop() {
        if !seen.insert(name) {
            continue;
        }
        if deny.contains(name) {
            return Some(path);
        }
        for dep in normal_deps(name) {
            stack.push((dep, format!("{path} -> {dep}")));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        toml::from_str(
            r#"
            ui_deny = ["winit", "wgpu", "egui"]
            third_party = ["serde", "wgpu", "middle"]
            [crates]
            low = "layer(0)"
            mid = "layer(1)"
            mid2 = "layer(1)"
            gpu = "layer(5)"
            native = "ffi(1)"
            shell = "app"
            kit = "testkit"
            task = "tool"
            [[same_layer]]
            from = "mid2"
            to = "mid"
            "#,
        )
        .unwrap()
    }

    fn krate(name: &str, deps: &[(&str, DepKind)]) -> Crate {
        Crate {
            name: name.to_owned(),
            declares_unsafe: false,
            deps: deps
                .iter()
                .map(|&(n, kind)| Dep {
                    name: n.to_owned(),
                    kind,
                })
                .collect(),
        }
    }

    fn rules(workspace: Vec<Crate>) -> Vec<Rule> {
        let facts = Facts {
            workspace,
            external: BTreeMap::new(),
        };
        check(&table(), &facts).iter().map(|v| v.rule).collect()
    }

    use DepKind::{Dev, Normal};

    #[test]
    fn downward_and_listed_edges_pass() {
        let workspace = vec![
            krate("low", &[("serde", Normal)]),
            krate("mid", &[("low", Normal)]),
            krate("mid2", &[("mid", Normal), ("kit", Dev)]),
            krate("shell", &[("gpu", Normal)]),
            krate("gpu", &[("wgpu", Normal)]),
            krate("kit", &[("mid", Normal)]),
            krate("task", &[]),
        ];
        assert_eq!(rules(workspace), vec![]);
    }

    #[test]
    fn upward_edge_fails() {
        let workspace = vec![krate("low", &[("mid", Normal)]), krate("mid", &[])];
        assert_eq!(rules(workspace), vec![Rule::Upward]);
    }

    #[test]
    fn upward_dev_edge_fails() {
        let workspace = vec![krate("low", &[("mid", Dev)]), krate("mid", &[])];
        assert_eq!(rules(workspace), vec![Rule::Upward]);
    }

    #[test]
    fn unlisted_same_layer_edge_fails() {
        let workspace = vec![krate("mid", &[("mid2", Normal)]), krate("mid2", &[])];
        assert_eq!(rules(workspace), vec![Rule::SameLayer]);
    }

    #[test]
    fn direct_ui_dependency_below_l5_fails() {
        assert_eq!(
            rules(vec![krate("mid", &[("wgpu", Normal)])]),
            vec![Rule::Ui]
        );
    }

    #[test]
    fn transitive_ui_dependency_below_l5_fails() {
        let mut facts = Facts {
            workspace: vec![krate("low", &[("middle", Normal)])],
            ..Facts::default()
        };
        facts
            .external
            .insert("middle".to_owned(), vec!["winit".to_owned()]);
        let found = check(&table(), &facts);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].rule, Rule::Ui);
        assert!(
            found[0].detail.contains("low -> middle -> winit"),
            "{}",
            found[0].detail
        );
    }

    #[test]
    fn ui_dev_dependency_is_not_a_ui_violation() {
        assert_eq!(rules(vec![krate("low", &[("wgpu", Dev)])]), vec![]);
    }

    #[test]
    fn unsafe_outside_ffi_crate_fails() {
        let mut low = krate("low", &[]);
        low.declares_unsafe = true;
        let mut native = krate("native", &[]);
        native.declares_unsafe = true;
        assert_eq!(rules(vec![low, native]), vec![Rule::Unsafe]);
    }

    #[test]
    fn dependency_on_app_or_tool_fails() {
        let workspace = vec![
            krate("gpu", &[("shell", Normal), ("task", Dev)]),
            krate("shell", &[]),
            krate("task", &[]),
        ];
        assert_eq!(rules(workspace), vec![Rule::NotALibrary, Rule::NotALibrary]);
    }

    #[test]
    fn normal_dependency_on_testkit_fails() {
        let workspace = vec![krate("mid", &[("kit", Normal)]), krate("kit", &[])];
        assert_eq!(rules(workspace), vec![Rule::NotALibrary]);
    }

    #[test]
    fn third_party_crate_outside_baseline_fails() {
        assert_eq!(
            rules(vec![krate("low", &[("leftpad", Normal)])]),
            vec![Rule::ThirdParty]
        );
    }

    #[test]
    fn unclassified_crate_fails() {
        assert_eq!(
            rules(vec![krate("stranger", &[])]),
            vec![Rule::Unclassified]
        );
    }

    #[test]
    fn malformed_class_fails() {
        let mut table = table();
        table.crates.insert("odd".to_owned(), "layer(x)".to_owned());
        let found = check(&table, &Facts::default());
        assert_eq!(
            found.iter().map(|v| v.rule).collect::<Vec<_>>(),
            vec![Rule::Table]
        );
    }

    #[test]
    fn repository_table_parses_and_has_only_known_classes() {
        let table: Table = toml::from_str(include_str!("../layers.toml")).unwrap();
        assert_eq!(check(&table, &Facts::default()), vec![]);
        assert_eq!(
            Class::parse(&table.crates["draftbuilder-model"]),
            Some(Class::Layer(0))
        );
    }
}
