## Why this fails

The step could not acquire the service-manager mutex. The current producer
reports cancellation while waiting, a poisoned mutex or expiry of the bounded
lock wait. Runtime startup and service-handle acquisition errors instead flow
through SERVICE_LEASE_FAILURE.

## Diagnose and repair

Inspect the original lock error and cancellation state. For a poisoned mutex,
inspect the earlier failure in the process. For lock contention, identify the
operation holding the manager and why it cannot finish. Do not delete external
resource locks or change runtime settings merely because this code says setup.

## Example

If verification was cancelled while waiting for the service-manager lock,
resolve the cancellation and inspect cleanup before rerunning. Installing a
runtime would not repair that cancellation. Likewise, a bounded lock wait
expiring requires investigation of the holder, not bypassing synchronization.
