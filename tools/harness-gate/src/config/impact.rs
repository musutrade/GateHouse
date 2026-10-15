//! Generic, explicit component influence. No language or plugin discovery.
use super::model::{ComponentImpact, FlowConfig};
use anyhow::{ensure, Context, Result};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

type Consumers = BTreeMap<String, BTreeSet<String>>;

impl FlowConfig {
    /// Reject incomplete/stale inventories and ambiguous declarations before
    /// scope can suppress work. Kahn's algorithm avoids recursive stack growth.
    pub(crate) fn validate_impact(&self) -> Result<()> {
        let Some(graph) = &self.scope.impact else {
            return Ok(());
        };
        ensure!(graph.version == 1, "scope.impact.version must be 1");
        validate_input_pins(graph)?;
        let degrees = validate_declarations(graph, &self.components())?;
        validate_acyclic(degrees, &self.impact_consumers())
    }

    pub(crate) fn validate_impact_inputs(&self, root: &Path) -> Result<()> {
        let Some(graph) = &self.scope.impact else {
            return Ok(());
        };
        for (name, expected) in &graph.inputs {
            let path = crate::project::resolve_repo_path(
                root,
                Path::new(name),
                "scope.impact input",
                true,
            )?;
            ensure!(
                path.is_file(),
                "scope.impact input {name:?} must be a regular text file"
            );
            let source = super::loader::read_config_source(&path)
                .with_context(|| format!("read scope.impact input {name:?}"))?;
            let actual = format!("{:x}", Sha256::digest(source.as_bytes()));
            ensure!(&actual == expected, "scope.impact input {name:?} is stale; review/regenerate the graph and its input pins, or remove impact selection and use --all");
        }
        Ok(())
    }

    fn impact_consumers(&self) -> Consumers {
        let mut consumers = Consumers::new();
        if let Some(graph) = &self.scope.impact {
            for (consumer, dependencies) in &graph.dependencies {
                for dependency in dependencies {
                    consumers
                        .entry(dependency.clone())
                        .or_default()
                        .insert(consumer.clone());
                }
            }
        }
        consumers
    }

    /// Breadth-first reachability. One deterministic shortest witness parent
    /// per component keeps normal selection linear in the reachable graph,
    /// including diamond graphs with exponentially many possible paths.
    pub(crate) fn impact_parents(&self, direct: &BTreeSet<String>) -> BTreeMap<String, String> {
        if self.scope.impact.is_none() || direct.is_empty() {
            return BTreeMap::new();
        }
        let consumers = self.impact_consumers();
        let mut selected = direct.clone();
        let mut pending: VecDeque<_> = direct.iter().cloned().collect();
        let mut parents = BTreeMap::new();
        while let Some(id) = pending.pop_front() {
            for consumer in consumers.get(&id).into_iter().flatten() {
                if selected.insert(consumer.clone()) {
                    parents.insert(consumer.clone(), id.clone());
                    pending.push_back(consumer.clone());
                }
            }
        }
        parents
    }

    pub(crate) fn expand_impact(&self, components: &mut BTreeSet<String>) {
        components.extend(self.impact_parents(components).into_keys());
    }
}

fn validate_input_pins(graph: &ComponentImpact) -> Result<()> {
    ensure!(
        graph.inputs.len() <= 64,
        "scope.impact exceeds 64 input pins"
    );
    for (path, digest) in &graph.inputs {
        super::validation::validate_impact_input_path(path)?;
        ensure!(
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "scope.impact input {path:?} requires a lowercase SHA-256 digest"
        );
    }
    Ok(())
}

fn validate_declarations(
    graph: &ComponentImpact,
    components: &BTreeSet<String>,
) -> Result<BTreeMap<String, usize>> {
    ensure!(
        graph.dependencies.len() <= 1024,
        "scope.impact exceeds 1024 components"
    );
    ensure!(graph.dependencies.keys().cloned().collect::<BTreeSet<_>>() == *components,
        "scope.impact.dependencies must declare exactly every workflow component (use [] for no dependencies); missing or stale component inventory");
    let mut degrees = BTreeMap::new();
    let mut edges = 0usize;
    for (consumer, dependencies) in &graph.dependencies {
        edges += dependencies.len();
        ensure!(edges <= 8192, "scope.impact exceeds 8192 edges");
        let mut unique = BTreeSet::new();
        for dependency in dependencies {
            ensure!(
                components.contains(dependency),
                "scope.impact component {consumer:?} references unknown dependency {dependency:?}"
            );
            ensure!(
                unique.insert(dependency),
                "scope.impact component {consumer:?} has duplicate dependency {dependency:?}"
            );
        }
        degrees.insert(consumer.clone(), dependencies.len());
    }
    Ok(degrees)
}

fn validate_acyclic(mut degrees: BTreeMap<String, usize>, consumers: &Consumers) -> Result<()> {
    let mut pending: VecDeque<_> = degrees
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect();
    let mut visited = 0;
    while let Some(id) = pending.pop_front() {
        visited += 1;
        for consumer in consumers.get(&id).into_iter().flatten() {
            let degree = degrees.get_mut(consumer).expect("validated component");
            *degree -= 1;
            if *degree == 0 {
                pending.push_back(consumer.clone());
            }
        }
    }
    ensure!(
        visited == degrees.len(),
        "scope.impact contains a component dependency cycle"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::model::ComponentImpact;
    use super::*;
    use std::time::Instant;

    fn fixture_graph(count: usize) -> FlowConfig {
        let mut config: FlowConfig = toml::from_str(include_str!(
            "../../fixtures/preset-compat/rust-api.flow.toml"
        ))
        .unwrap();
        let step = config.steps[0].clone();
        config.steps = (0..count)
            .map(|index| {
                let mut step = step.clone();
                step.id = format!("c{index}.test");
                step.component = format!("c{index}");
                step
            })
            .collect();
        config.scope.impact = Some(ComponentImpact {
            version: 1,
            inputs: BTreeMap::new(),
            dependencies: (0..count)
                .map(|index| {
                    (
                        format!("c{index}"),
                        if index == 0 {
                            vec![]
                        } else {
                            vec![format!("c{}", index - 1)]
                        },
                    )
                })
                .collect(),
        });
        config
    }

    #[test]
    fn diamond_and_multiple_roots_have_deterministic_shortest_witnesses() {
        let mut config = fixture_graph(5);
        let graph = config.scope.impact.as_mut().unwrap();
        graph.dependencies.insert("c2".into(), vec!["c0".into()]);
        graph
            .dependencies
            .insert("c3".into(), vec!["c2".into(), "c1".into()]);
        graph.dependencies.insert("c4".into(), vec![]);
        config.validate_impact().unwrap();
        let direct = BTreeSet::from(["c0".into(), "c4".into()]);
        let parents = config.impact_parents(&direct);
        assert_eq!(parents["c3"], "c1");
        assert_eq!(parents.len(), 3);
        let multiple = config.impact_parents(&BTreeSet::from(["c0".into(), "c3".into()]));
        assert!(!multiple.contains_key("c3"));
        config.scope.impact = None;
        assert!(config.impact_parents(&direct).is_empty());
    }

    #[test]
    fn graph_budgets_and_invalid_pins_fail_closed() {
        assert!(fixture_graph(1025)
            .validate_impact()
            .unwrap_err()
            .to_string()
            .contains("1024"));
        let mut config = fixture_graph(130);
        let graph = config.scope.impact.as_mut().unwrap();
        for index in 0..130 {
            graph.dependencies.insert(
                format!("c{index}"),
                (0..index).map(|n| format!("c{n}")).collect(),
            );
        }
        assert!(config
            .validate_impact()
            .unwrap_err()
            .to_string()
            .contains("8192"));
        let mut config = fixture_graph(1);
        config.scope.impact.as_mut().unwrap().inputs = (0..65)
            .map(|n| (format!("file{n}"), "0".repeat(64)))
            .collect();
        assert!(config
            .validate_impact()
            .unwrap_err()
            .to_string()
            .contains("64 input"));
        config.scope.impact.as_mut().unwrap().inputs =
            BTreeMap::from([("C:\\escape".into(), "0".repeat(64))]);
        assert!(config.validate_impact().is_err());
    }

    #[test]
    fn thousand_component_chain_selects_consumers_without_recursive_or_path_explosion() {
        let config = fixture_graph(1000);
        config.validate_impact().unwrap();
        let seed = BTreeSet::from(["c0".into()]);
        let start = Instant::now();
        for _ in 0..100 {
            let mut selected = seed.clone();
            config.expand_impact(&mut selected);
            assert_eq!(selected.len(), 1000);
        }
        eprintln!(
            "1000 components, 999 edges, 100 selections: {} us",
            start.elapsed().as_micros()
        );
        let mut leaf = BTreeSet::from(["c999".into()]);
        config.expand_impact(&mut leaf);
        assert_eq!(leaf.len(), 1);
    }
}
