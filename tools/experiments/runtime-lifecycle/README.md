# Lifecycle boundary design experiment

Run on Linux with Python 3.12 or newer from the repository root:

```sh
python3 tools/experiments/runtime-lifecycle/run.py
```

This reproduces the isolated experiment described in
[the boundary proposal](../../../docs/runtime-plugin-boundary.md).
The runner copies source into temporary storage, executes each check with an
outer timeout, and removes generated state/reports afterward. It invokes no
Docker/Podman daemon and does not alter Core configuration or required CI.
Do not run the individual check scripts in the checkout: they write local JSON
summaries next to their source. Use the temporary runner above.

`host.py` models a lease coordinator; `adapter.py` is a filesystem fake engine.
The four checks exercise subprocess recovery, cooperative locking, fixed artifact
identity, Linux process groups, and direct response validation. Simulated owner
facts and namespaces do not certify production identity or daemon behavior.
No result archives or state files belong in this directory.
