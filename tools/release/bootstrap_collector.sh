#!/bin/sh
# Provision this script AND the bootstrap digest independently of the collector.
# The authenticated bootstrap carries private Python and reviewed installer code.
set -eu
umask 077
unset PYTHONHOME PYTHONPATH PYTHONSTARTUP LD_PRELOAD LD_LIBRARY_PATH
export PATH=/usr/bin:/bin
if [ "$#" -lt 3 ]; then
    echo 'usage: bootstrap_collector.sh TRUSTED_SHA256 BOOTSTRAP_TAR INSTALLER_ARGS...' >&2
    exit 2
fi
digest=$1
archive=$2
shift 2
case "$digest" in ''|*[!0-9a-f]*) echo 'invalid independent bootstrap digest' >&2; exit 2;; esac
[ "${#digest}" -eq 64 ] || exit 2
[ -f "$archive" ] && [ ! -L "$archive" ] || { echo 'missing/unsafe bootstrap' >&2; exit 2; }
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT HUP INT TERM
# Verify a private snapshot, then extract that same snapshot (no pathname race).
cp -- "$archive" "$stage/bootstrap.tar"
actual=$(sha256sum "$stage/bootstrap.tar")
[ "${actual%% *}" = "$digest" ] || { echo 'untrusted bootstrap digest' >&2; exit 1; }
mkdir "$stage/runtime"
# Tar and shell are administrator-authenticated host inputs. No archive byte is
# interpreted before its independent digest matches. Builder emits regular files.
tar --extract --file "$stage/bootstrap.tar" --directory "$stage/runtime" --no-same-owner --no-same-permissions
export LD_LIBRARY_PATH="$stage/runtime/lib"
"$stage/runtime/python/bin/python3" -I -S -B "$stage/runtime/tools/release/production_installer.py" "$@"
