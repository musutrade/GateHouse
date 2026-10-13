use super::plan::VerificationPlan;
use crate::config::WaiverConfig;
use crate::process::{TaskResult, WaiverEvidence};
use crate::project::Project;
use crate::scope::ScopeResult;
use anyhow::{bail, Result};
use chrono::Utc;

pub(super) fn validate_plan_waivers(
    project: &Project,
    plan: &VerificationPlan<'_>,
    scope: &ScopeResult,
) -> Result<()> {
    for node in &plan.nodes {
        let Some(step) = node.step else { continue };
        let Some(waiver) = project
            .config
            .policy
            .waivers
            .iter()
            .find(|waiver| waiver.step == step.id)
        else {
            continue;
        };
        if waiver.revoked {
            bail!("waiver {:?} for step {:?} is revoked", waiver.id, step.id);
        }
        if !scope_matches(waiver, scope) {
            bail!(
                "waiver {:?} for step {:?} is outside the selected scope",
                waiver.id,
                step.id
            );
        }
        let expiry = chrono::DateTime::parse_from_rfc3339(&waiver.expires_at)?;
        if expiry.with_timezone(&Utc) <= Utc::now() {
            bail!("waiver {:?} for step {:?} has expired", waiver.id, step.id);
        }
    }
    Ok(())
}

pub(super) fn apply(
    project: &Project,
    step_id: &str,
    scope: &ScopeResult,
    result: &mut TaskResult,
) {
    if result.passed || result.cancelled {
        return;
    }
    let Some(waiver) =
        project.config.policy.waivers.iter().find(|waiver| {
            waiver.step == step_id && !waiver.revoked && scope_matches(waiver, scope)
        })
    else {
        return;
    };
    let Ok(expiry) = chrono::DateTime::parse_from_rfc3339(&waiver.expires_at) else {
        return;
    };
    if expiry.with_timezone(&Utc) <= Utc::now() {
        return;
    }
    result.passed = true;
    result.waived = true;
    result.waiver = Some(WaiverEvidence {
        id: waiver.id.clone(),
        risk: waiver.risk.clone(),
        owner: waiver.owner.clone(),
        approved_by: waiver.approved_by.clone(),
        created_at: waiver.created_at.clone(),
        expires_at: waiver.expires_at.clone(),
        compensating_control: waiver.compensating_control.clone(),
    });
    result.detail = Some(format!(
        "waived by {} until {}",
        waiver.id, waiver.expires_at
    ));
}

fn scope_matches(waiver: &WaiverConfig, scope: &ScopeResult) -> bool {
    waiver.scope.as_deref().is_none_or(|expected| {
        expected == "all"
            || expected == scope.mode
            || scope.components.contains(expected)
            || scope.changed_files.iter().any(|path| path == expected)
    })
}
