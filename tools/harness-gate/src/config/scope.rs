use super::model::FlowConfig;
use anyhow::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::collections::BTreeSet;

/// Compiled scope matchers reused for the lifetime of a discovered project.
/// Keeping the compiled globsets outside the serialized configuration avoids
/// rebuilding them for every scope/verify invocation.
#[derive(Debug, Clone)]
pub(crate) struct CompiledScopeRules {
    matchers: Vec<GlobSet>,
}

impl CompiledScopeRules {
    pub(crate) fn matching_rules(&self, path: &str) -> Vec<usize> {
        self.matchers
            .iter()
            .enumerate()
            .filter_map(|(index, matcher)| matcher.is_match(path).then_some(index))
            .collect()
    }
    pub(crate) fn compile(config: &FlowConfig) -> Result<Self> {
        config.validate_impact()?;
        let matchers = config
            .scope
            .rules
            .iter()
            .map(|rule| {
                let mut builder = GlobSetBuilder::new();
                for pattern in &rule.patterns {
                    builder.add(Glob::new(pattern)?);
                }
                Ok(builder.build()?)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { matchers })
    }

    fn classify(&self, config: &FlowConfig, paths: &[String]) -> (BTreeSet<String>, Vec<String>) {
        let mut components = BTreeSet::new();
        let mut unmatched = Vec::new();
        for path in paths {
            let mut matched = false;
            for index in self.matching_rules(path) {
                matched = true;
                components.extend(config.scope.rules[index].components.iter().cloned());
            }
            if !matched {
                unmatched.push(path.clone());
            }
        }
        config.expand_impact(&mut components);
        (components, unmatched)
    }
}

impl FlowConfig {
    /// Classify paths with matchers compiled for this call.
    ///
    /// Project discovery uses [`Self::compile_scope_rules`] and retains the
    /// result so repeated commands do not rebuild the globsets.
    pub fn classify_paths(&self, paths: &[String]) -> Result<(BTreeSet<String>, Vec<String>)> {
        let matchers = self.compile_scope_rules()?;
        Ok(matchers.classify(self, paths))
    }

    pub(crate) fn compile_scope_rules(&self) -> Result<CompiledScopeRules> {
        CompiledScopeRules::compile(self)
    }

    pub(crate) fn classify_paths_with(
        &self,
        matchers: &CompiledScopeRules,
        paths: &[String],
    ) -> (BTreeSet<String>, Vec<String>) {
        matchers.classify(self, paths)
    }
}
