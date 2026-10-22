//! Separate operator path. Normal service startup/Drop never calls this code.
use super::*;
use gatehouse_runtime_protocol::lifecycle::{journal::OperatorRecovery, state::Phase};
fn run(
    recovery: &mut OperatorRecovery,
    installation: &VerifiedInstallation,
    operation: Operation,
    deadline: Instant,
    cancelled: &impl Fn() -> bool,
) -> Result<Response> {
    recovery.check()?;
    let timeout = remaining(deadline)?;
    let invocation = nonce()?;
    let response = outcome(host::dispatch(
        recovery.begin(operation, &invocation)?,
        installation,
        timeout,
        cancelled,
    ))?;
    recovery.check()?;
    Ok(response)
}
fn status(
    recovery: &mut OperatorRecovery,
    installation: &VerifiedInstallation,
    deadline: Instant,
    cancelled: &impl Fn() -> bool,
) -> Result<Response> {
    let target = recovery.target()?;
    run(
        recovery,
        installation,
        Operation::Status { target },
        deadline,
        cancelled,
    )
}
fn stop_running(
    recovery: &mut OperatorRecovery,
    installation: &VerifiedInstallation,
    deadline: Instant,
    cancelled: &impl Fn() -> bool,
) -> Result<()> {
    let target = recovery.target()?;
    run(
        recovery,
        installation,
        Operation::Stop {
            target,
            grace_seconds: 5,
        },
        deadline,
        cancelled,
    )?;
    match status(recovery, installation, deadline, cancelled)?.outcome {
        Outcome::Status { object }
            if matches!(
                object.state,
                ContainerState::Created | ContainerState::Exited
            ) => {}
        _ => bail!("operator stop not proved; retain slot"),
    }
    Ok(())
}
/// Requires an exact-fence operator token. Recovered unknown-ID creation is never
/// adopted by name. Only exact-ID status/stop/remove/absence are available.
pub(crate) fn cleanup_known(
    mut recovery: OperatorRecovery,
    installation: VerifiedInstallation,
    deadline: Instant,
    cancelled: impl Fn() -> bool,
) -> Result<()> {
    recovery.check()?;
    installation.validate_request(recovery.resolve_request())?;
    remaining(deadline)?;
    if cancelled() {
        bail!("operator recovery cancelled; retain slot");
    }
    if recovery.phase() == Phase::VerifiedAbsent {
        recovery.finish()?;
        return Ok(());
    }
    let observed = status(&mut recovery, &installation, deadline, &cancelled)?;
    match observed.outcome {
        Outcome::Absent { .. } => {
            recovery.finish()?;
            return Ok(());
        }
        Outcome::Status { object } if object.state == ContainerState::Running => {
            stop_running(&mut recovery, &installation, deadline, &cancelled)?;
        }
        Outcome::Status { object }
            if matches!(
                object.state,
                ContainerState::Created | ContainerState::Exited
            ) => {}
        _ => bail!("operator recovery state unsupported; retain slot"),
    }
    let target = recovery.target()?;
    run(
        &mut recovery,
        &installation,
        Operation::Remove { target },
        deadline,
        &cancelled,
    )?;
    if !matches!(
        status(&mut recovery, &installation, deadline, &cancelled)?.outcome,
        Outcome::Absent { .. }
    ) {
        bail!("operator removal absence not proved; retain slot");
    }
    recovery.finish()?;
    Ok(())
}
