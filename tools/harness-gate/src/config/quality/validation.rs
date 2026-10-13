use super::model::*;
use crate::{config::FlowConfig, project::resolve_repo_path};
use anyhow::{bail, ensure, Context, Result};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

impl QualityConfig {
    pub(super) fn validate(&self, flow: &FlowConfig, root: &Path) -> Result<()> {
        let canonical_root = root
            .canonicalize()
            .context("resolve quality repository root")?;
        let root = canonical_root.as_path();
        ensure!(self.version == 1, "quality.version must be 1");
        if let Some(limits) = &self.limits {
            ensure!(
                limits.max_artifact_bytes > 0,
                "quality.limits.max_artifact_bytes must be positive"
            );
        }
        identifier(&self.project.id)?;
        ensure!(
            self.project.name == flow.project.name,
            "quality.project.name must match flow.project.name"
        );
        ensure!(
            !self.components.is_empty(),
            "quality.components must not be empty"
        );
        for id in self
            .components
            .keys()
            .chain(self.subjects.keys())
            .chain(self.relationships.keys())
            .chain(self.collectors.keys())
            .chain(self.policies.keys())
            .chain(self.profiles.keys())
        {
            identifier(id)?;
        }
        self.validate_components(flow, root)?;
        self.validate_subjects(root)?;
        for relationship in self.relationships.values() {
            identifier(&relationship.kind)?;
            for endpoint in [&relationship.from, &relationship.to] {
                ensure!(
                    !matches!(endpoint, Target::Relationship { .. }),
                    "relationship endpoints must be components or subjects"
                );
                self.validate_target(endpoint)?;
            }
            ensure!(
                relationship.from != relationship.to,
                "relationship endpoints must differ"
            );
        }
        for collector in self.collectors.values() {
            path(root, &collector.request, "collector request", false)?;
            ensure!(
                !collector.produces.is_empty(),
                "collector.produces must not be empty"
            );
            let mut targets = BTreeSet::new();
            for expectation in &collector.produces {
                self.validate_expectation(expectation)?;
                ensure!(
                    targets.insert((&expectation.target, &expectation.capability)),
                    "duplicate collector capability for target"
                );
            }
        }
        for profile in self.profiles.values() {
            if let Some(workflow) = &profile.workflow {
                for reference in [
                    Some(&workflow.state),
                    Some(&workflow.trusted_keys),
                    workflow.baseline_request.as_ref(),
                ]
                .into_iter()
                .flatten()
                {
                    path(root, reference, "workflow input", false)?;
                }
            }
        }
        self.validate_baseline(root)?;
        let requirements = self
            .policies
            .iter()
            .map(|(id, binding)| {
                self.validate_expectation(&binding.expectation)?;
                let required = super::policy::validate_binding(self, binding, root)?;
                Ok((id.as_str(), required))
            })
            .collect::<Result<std::collections::BTreeMap<_, _>>>()?;
        self.validate_profiles(flow, &requirements)?;
        path(root, &self.reporting.output, "reporting.output", false)?;
        ensure!(
            !self.reporting.formats.is_empty(),
            "reporting.formats must not be empty"
        );
        Ok(())
    }

    fn validate_components(&self, flow: &FlowConfig, root: &Path) -> Result<()> {
        let flow_components = flow.components();
        for (id, component) in &self.components {
            ensure!(
                !component.flow_components.is_empty()
                    && component.flow_components.is_subset(&flow_components),
                "component {id} references missing flow components"
            );
            ensure!(
                !component.source_roots.is_empty(),
                "component {id} needs source roots"
            );
            for source in &component.source_roots {
                path(root, source, "component source root", false)?;
            }
            path(
                root,
                &component.artifact_root,
                "component artifact root",
                false,
            )?;
        }
        Ok(())
    }

    fn validate_subjects(&self, root: &Path) -> Result<()> {
        for (id, subject) in &self.subjects {
            let component = self
                .components
                .get(&subject.component)
                .with_context(|| format!("subject {id} references unknown component"))?;
            identifier(&subject.kind)?;
            match &subject.selection {
                SubjectSelection::Explicit { paths } => {
                    ensure!(!paths.is_empty(), "subject {id} needs explicit paths");
                    let roots = component
                        .source_roots
                        .iter()
                        .map(|source| path(root, source, "source root", false))
                        .collect::<Result<Vec<_>>>()?;
                    for source in paths {
                        let resolved = path(root, source, "subject path", false)?;
                        ensure!(
                            roots.iter().any(|boundary| resolved.starts_with(boundary)),
                            "subject {id} escapes component source roots"
                        );
                    }
                }
                SubjectSelection::Discovery { collector, query } => {
                    ensure!(
                        self.collectors.contains_key(collector),
                        "subject {id} references unknown discovery collector"
                    );
                    ensure!(
                        !query.trim().is_empty(),
                        "subject {id} discovery query must not be empty"
                    );
                }
            }
        }
        Ok(())
    }

    pub(super) fn validate_target(&self, target: &Target) -> Result<()> {
        let found = match target {
            Target::Component { id } => self.components.contains_key(id),
            Target::Subject { id } => self.subjects.contains_key(id),
            Target::Relationship { id } => self.relationships.contains_key(id),
        };
        ensure!(found, "unresolved quality target {target:?}");
        Ok(())
    }

    fn validate_expectation(&self, expectation: &Expectation) -> Result<()> {
        self.validate_target(&expectation.target)?;
        let metric = &expectation.capability;
        ensure!(
            metric.contains('.')
                && metric.split('.').all(|part| !part.is_empty()
                    && part.bytes().enumerate().all(|(i, c)| c.is_ascii_lowercase()
                        || (i > 0 && (c.is_ascii_digit() || c == b'_')))),
            "invalid capability {metric}"
        );
        let digest = expectation
            .series
            .strip_prefix("measurement-series/v1:")
            .unwrap_or("");
        ensure!(
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
            "invalid measurement series identity"
        );
        Ok(())
    }

    fn validate_baseline(&self, root: &Path) -> Result<()> {
        match &self.baseline.provider {
            BaselineProvider::None => {
                ensure!(!self.baseline.required, "required baseline has no provider")
            }
            BaselineProvider::Git { reference, .. } => ensure!(
                !reference.trim().is_empty()
                    && !reference.starts_with('-')
                    && !reference.chars().any(char::is_control),
                "invalid baseline Git reference"
            ),
            BaselineProvider::RetainedArtifact { manifest } => {
                path(root, manifest, "baseline manifest", false)?;
            }
        }
        Ok(())
    }

    fn validate_profiles(
        &self,
        flow: &FlowConfig,
        requirements: &std::collections::BTreeMap<&str, bool>,
    ) -> Result<()> {
        let declared: BTreeSet<_> = flow
            .steps
            .iter()
            .flat_map(|step| step.profiles.iter())
            .collect();
        ensure!(
            !self.profiles.is_empty(),
            "quality.profiles must not be empty"
        );
        for (name, profile) in &self.profiles {
            if profile.assurance == Assurance::Complete {
                for (id, required) in requirements {
                    ensure!(
                        !required || profile.policies.contains(*id),
                        "complete profile {name} omits required policy {id}"
                    );
                }
            }
            ensure!(
                declared.contains(name),
                "quality profile {name} is not declared in flow steps"
            );
            let mut producers = std::collections::BTreeMap::new();
            for id in &profile.collectors {
                let collector = self
                    .collectors
                    .get(id)
                    .with_context(|| format!("profile {name} references unknown collector {id}"))?;
                for production in &collector.produces {
                    ensure!(
                        producers
                            .insert(
                                (&production.target, &production.capability),
                                &production.series
                            )
                            .is_none(),
                        "profile {name} has conflicting authoritative producers"
                    );
                    self.validate_discovery(&production.target, profile)?;
                }
            }
            for id in &profile.policies {
                let binding = self
                    .policies
                    .get(id)
                    .with_context(|| format!("profile {name} references unknown policy {id}"))?;
                let expected = &binding.expectation;
                if let Some(series) = producers.get(&(&expected.target, &expected.capability)) {
                    ensure!(
                        **series == expected.series,
                        "profile {name} policy {id} has incompatible series"
                    );
                    self.validate_discovery(&expected.target, profile)?;
                } else {
                    ensure!(
                        !requirements[id.as_str()],
                        "profile {name} required policy {id} has no producer"
                    );
                }
            }
        }
        Ok(())
    }

    fn validate_discovery(&self, target: &Target, profile: &Participation) -> Result<()> {
        match target {
            Target::Subject { id } => {
                if let SubjectSelection::Discovery { collector, .. } = &self.subjects[id].selection
                {
                    ensure!(
                        profile.collectors.contains(collector),
                        "profile omits discovery collector {collector}"
                    );
                }
            }
            Target::Relationship { id } => {
                let relationship = &self.relationships[id];
                self.validate_discovery(&relationship.from, profile)?;
                self.validate_discovery(&relationship.to, profile)?;
            }
            Target::Component { id } => {
                for (subject_id, subject) in &self.subjects {
                    if &subject.component == id {
                        self.validate_discovery(
                            &Target::Subject {
                                id: subject_id.clone(),
                            },
                            profile,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }
}

pub(super) fn path(root: &Path, value: &str, label: &str, exists: bool) -> Result<PathBuf> {
    ensure!(
        !value.contains('\\')
            && !value.contains(':')
            && !value.contains("${")
            && !value.chars().any(char::is_control),
        "{label} must be a literal portable repository-relative path"
    );
    resolve_repo_path(root, Path::new(value), label, exists)
}

fn identifier(value: &str) -> Result<()> {
    if value.is_empty()
        || !value.bytes().enumerate().all(|(i, c)| {
            c.is_ascii_lowercase() || (i > 0 && (c.is_ascii_digit() || b"._-".contains(&c)))
        })
    {
        bail!("invalid quality identifier {value:?}");
    }
    Ok(())
}
