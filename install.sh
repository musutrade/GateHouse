#!/usr/bin/env bash
# Install a Harness-Gate binary from one immutable GitHub release tag.

set -Eeuo pipefail
umask 022

REPO="musutrade/Harness-Gate"
COLLECTOR_REPO="musutrade/GateProbes"
BINARY_NAME="harness-gate"
RELEASE_BASE_URL="https://github.com/${REPO}/releases/download"
COLLECTOR_RELEASE_BASE_URL="https://github.com/${COLLECTOR_REPO}/releases/download"
VERSION="${HARNESS_GATE_VERSION:-}"
INSTALL_DIR="${INSTALL_DIR:-${HOME}/.local/bin}"
OS=""
ARCH=""
PLATFORM=""
INSTALL_NAME="$BINARY_NAME"
ATOMIC_TEMPORARY=""
INSTALL_TEMPORARY_ROOT=""
RELEASE_WORKFLOW="release.yml"
RUST_VERSION="${HARNESS_GATE_RUST_VERSION:-0.1.0-rc.8}"
RUST_INSTALL_DIR=""
VERIFIER_CACHE_DIR="${HOME}/.cache/harness-gate/collector"
STAGED_BINARY=""
RUST_STAGED_BINARY=""

usage() {
    cat <<'EOF'
Usage: install.sh --version vX.Y.Z [--install-dir DIR]
       install.sh --version vX.Y.Z --from-source [--install-dir DIR]
       install.sh --version vX.Y.Z --with-rust [--rust-version X.Y.Z] [--install-dir DIR]
       install.sh --rust-only [--rust-version X.Y.Z] [--install-dir DIR]

The Core version is required unless --rust-only is selected. Every download is
bound to an immutable release tag. Core versions through v0.4.8 retain the
Harness-Gate repository and signer; v0.4.9 and later use GateHouse.
The installer verifies SHA256 and the Sigstore keyless certificate before it
changes the destination directory.
The optional Rust collector is a standalone binary published from the
GateProbes repository and verified against that repository's release workflow
identity. It defaults to 0.1.0-rc.8; --rust-version selects another exact
version from the same repository. Rust 1.97.1, matching LLVM tools and
Python 3.12+ remain external dependencies.
--rust-root DIR installs only the collector executable in DIR/bin.
--cache-dir DIR selects an existing checksum-pinned verifier cache.
Legacy bundled-runtime --offline archives require their historical installer.
EOF
}

die() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

cleanup_atomic_temporary() {
    if [[ -n "${ATOMIC_TEMPORARY:-}" ]]; then
        rm -f -- "$ATOMIC_TEMPORARY" 2>/dev/null || true
        ATOMIC_TEMPORARY=""
    fi
}

cleanup_installation() {
    local status=$?
    trap - EXIT
    cleanup_atomic_temporary
    if [[ -n "$INSTALL_TEMPORARY_ROOT" ]]; then
        rm -rf -- "$INSTALL_TEMPORARY_ROOT" 2>/dev/null || true
        INSTALL_TEMPORARY_ROOT=""
    fi
    exit "$status"
}

abort_on_signal() {
    local status="$1"
    cleanup_atomic_temporary
    exit "$status"
}

validate_version() {
    local version="${1:-$VERSION}"
    [[ "$version" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$ ]] \
        || die "version must be an exact v-prefixed SemVer (for example v0.3.3)"
    if [[ "$version" == *-* ]]; then
        local prerelease="${version#*-}"
        prerelease="${prerelease%%+*}"
        local identifier
        IFS='.' read -r -a identifiers <<<"$prerelease"
        for identifier in "${identifiers[@]}"; do
            [[ ! "$identifier" =~ ^0[0-9]+$ ]] \
                || die "numeric prerelease identifiers must not contain leading zeroes"
        done
    fi
}

select_core_repository() {
    # Historical versions keep their original download and certificate identity.
    # This reviewed cutover is script-owned, never an environment URL override.
    if [[ "$VERSION" =~ ^v0\.([0-3]\.[0-9]+|4\.[0-8])(-|\+|$) ]]; then
        REPO="musutrade/Harness-Gate"
    else
        REPO="musutrade/GateHouse"
    fi
    RELEASE_BASE_URL="https://github.com/${REPO}/releases/download"
}

detect_platform() {
    local os
    local arch
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    arch="$(uname -m)"

    case "$os" in
        linux*) OS="linux" ;;
        darwin*) OS="macos" ;;
        mingw*|msys*|cygwin*) OS="windows" ;;
        *) die "unsupported operating system: $os" ;;
    esac

    case "$arch" in
        x86_64|amd64) ARCH="amd64" ;;
        arm64|aarch64) ARCH="arm64" ;;
        *) die "unsupported architecture: $arch" ;;
    esac

    PLATFORM="${OS}-${ARCH}"
    INSTALL_NAME="$BINARY_NAME"
    if [[ "$OS" == windows ]]; then
        INSTALL_NAME="${BINARY_NAME}.exe"
    fi
}

download() {
    local url="$1"
    local output="$2"
    if command -v curl >/dev/null 2>&1; then
        curl --fail --show-error --location --proto '=https' --tlsv1.2 \
            --max-redirs 10 --retry 3 --retry-all-errors --output "$output" "$url"
    elif command -v wget >/dev/null 2>&1; then
        # TLSv1_2 rejects SSLv3/TLS 1.0/1.1 the same way curl's --tlsv1.2 does.
        wget --https-only --secure-protocol=TLSv1_2 --max-redirect=10 \
            --tries=3 --output-document="$output" "$url"
    else
        die "curl or wget is required"
    fi
    [[ ! -L "$output" && -f "$output" && -s "$output" ]] \
        || die "downloaded file is missing, symlinked, or empty: $url"
}

verify_checksum() {
    local dist="$1"
    local filename="$2"
    local manifest="$dist/SHA256SUMS"
    local selected="$dist/checksum.selected"
    local matches

    matches="$(LC_ALL=C awk -v expected="$filename" '$2 == expected { print; count++ } END { if (count != 1) exit 1 }' "$manifest")" \
        || die "SHA256SUMS does not contain exactly one entry for $filename"
    printf '%s\n' "$matches" >"$selected"
    if command -v sha256sum >/dev/null 2>&1; then
        (cd "$dist" && sha256sum --check --status "$(basename "$selected")") \
            || die "SHA256 checksum verification failed for $filename"
    elif command -v shasum >/dev/null 2>&1; then
        (cd "$dist" && shasum -a 256 --check --status "$(basename "$selected")") \
            || die "SHA256 checksum verification failed for $filename"
    else
        die "sha256sum or shasum is required"
    fi
}

verify_signature() {
    local dist="$1"
    local filename="$2"
    local identity_repo="$3"
    command -v cosign >/dev/null 2>&1 \
        || die "cosign is required for Sigstore verification (see https://docs.sigstore.dev/cosign/system_config/installation/)"

    # The release workflow's OIDC identity is bound to this exact immutable tag.
    # Core releases verify the Harness-Gate workflow; the Rust collector verifies
    # the explicitly provisioned GateProbes workflow. The expected signer is a
    # script-owned constant passed by the caller; it is never read from the
    # calling environment, so an exported lookalike variable cannot change it.
    cosign verify-blob \
        --signature "$dist/${filename}.sig" \
        --certificate "$dist/${filename}.crt" \
        --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
        --certificate-identity "https://github.com/${identity_repo}/.github/workflows/${RELEASE_WORKFLOW}@refs/tags/${VERSION}" \
        "$dist/$filename" \
        || die "Sigstore verification failed for $filename"
}

ensure_cosign() {
    command -v cosign >/dev/null 2>&1 && return 0
    local name digest path
    case "$PLATFORM" in
        linux-amd64) name=cosign-linux-amd64; digest=4629c757b7618056f8ddd7e2625ae9fdd94c0372a65049520bc7d9df9efc7f71 ;;
        macos-amd64) name=cosign-darwin-amd64; digest=2347488e5d5b25336644024dfeca5601b190e91197a71a917bda44744aff106c ;;
        macos-arm64) name=cosign-darwin-arm64; digest=5cf948c2f4dfe59687bdd0b8523709067383e03982cc543475c8a7dc70e92a76 ;;
        windows-amd64) name=cosign-windows-amd64.exe; digest=9fe59be0eca1271873ce019061335eb1ac419b7059202e797828467ddabe33be ;;
        *) die "no pinned signature verifier for $PLATFORM" ;;
    esac
    mkdir -p "$1/verifier"
    path="$1/verifier/cosign"
    [[ "$OS" != windows ]] || path="${path}.exe"
    local cached="$VERIFIER_CACHE_DIR/${digest}-${name}"
    if [[ -f "$cached" && ! -L "$cached" ]]; then
        cp -- "$cached" "$path"
    else
        download "https://github.com/sigstore/cosign/releases/download/v3.1.3/$name" "$path"
    fi
    local actual
    if command -v sha256sum >/dev/null 2>&1; then actual=$(sha256sum "$path"); else actual=$(shasum -a 256 "$path"); fi
    [[ "${actual%% *}" == "$digest" ]] || die "signature verifier checksum mismatch"
    chmod 755 "$path"
    export PATH="$1/verifier:$PATH"
}

prepare_rust() {
    local temporary="$1"
    local VERSION="rust-collector-v${RUST_VERSION}"
    local RELEASE_WORKFLOW="native-collector-release.yml"
    local RELEASE_BASE_URL="$COLLECTOR_RELEASE_BASE_URL"
    local SIGNER_REPO="$COLLECTOR_REPO"
    local BINARY_NAME="harness-gate-rust-collector"
    prepare_binary "$temporary"
    RUST_STAGED_BINARY="$STAGED_BINARY"
}

install_rust() {
    local VERSION="rust-collector-v${RUST_VERSION}"
    local BINARY_NAME="harness-gate-rust-collector"
    local INSTALL_NAME="$BINARY_NAME"
    local INSTALL_DIR="${RUST_INSTALL_DIR:-$INSTALL_DIR}"
    [[ "$OS" != windows ]] || INSTALL_NAME="${INSTALL_NAME}.exe"
    atomic_install "$RUST_STAGED_BINARY"
}

validate_install_dir() {
    [[ "$INSTALL_DIR" = /* ]] || die "install directory must be an absolute path: $INSTALL_DIR"
    [[ "$INSTALL_DIR" != / ]] || die "install directory must not be the filesystem root"
    [[ "$INSTALL_DIR" != *$'\n'* && "$INSTALL_DIR" != *$'\r'* ]] \
        || die "install directory contains a line break"
    local component
    local current="/"
    local relative="${INSTALL_DIR#/}"
    IFS='/' read -r -a components <<<"$relative"
    for component in "${components[@]}"; do
        [[ -n "$component" && "$component" != "." && "$component" != ".." ]] \
            || die "install directory must not contain empty, dot, or traversal components"
        current="${current%/}/$component"
        [[ ! -L "$current" ]] || die "install directory contains a symlink component: $current"
    done
    if [[ -e "$INSTALL_DIR" && ! -d "$INSTALL_DIR" ]]; then
        die "install directory is not a directory: $INSTALL_DIR"
    fi
    if [[ ! -e "$INSTALL_DIR" ]]; then
        mkdir -p "$INSTALL_DIR"
        chmod 0755 "$INSTALL_DIR"
    fi
    [[ ! -L "$INSTALL_DIR" ]] || die "install directory must not be a symlink: $INSTALL_DIR"
    [[ -d "$INSTALL_DIR" && -w "$INSTALL_DIR" ]] || die "install directory is not writable: $INSTALL_DIR"

    # Do not install into a directory writable by group or other users.
    local mode
    if mode="$(stat -c '%a' "$INSTALL_DIR" 2>/dev/null)"; then
        :
    else
        mode="$(stat -f '%Lp' "$INSTALL_DIR" 2>/dev/null)" \
            || die "cannot inspect install directory permissions: $INSTALL_DIR"
    fi
    [[ "$mode" =~ ^[0-7]+$ ]] || die "cannot inspect install directory permissions: $INSTALL_DIR"
    local mode_number=$((8#$mode))
    (( (mode_number & 8#022) == 0 )) || die "install directory is group/other writable: $INSTALL_DIR"

    local target="$INSTALL_DIR/$INSTALL_NAME"
    [[ ! -L "$target" ]] || die "refusing to replace symlink target: $target"
    if [[ -e "$target" && ! -f "$target" ]]; then
        die "existing install target is not a regular file: $target"
    fi
}

atomic_install() {
    local source="$1"
    [[ ! -L "$source" && -f "$source" ]] || die "installation source is not a regular file: $source"
    validate_install_dir
    local target="$INSTALL_DIR/$INSTALL_NAME"
    ATOMIC_TEMPORARY="$(mktemp "$INSTALL_DIR/.${INSTALL_NAME}.XXXXXX")" \
        || die "cannot allocate an atomic install file in $INSTALL_DIR"
    if ! cp "$source" "$ATOMIC_TEMPORARY"; then
        cleanup_atomic_temporary
        die "cannot stage binary in $INSTALL_DIR"
    fi
    if ! chmod 0755 "$ATOMIC_TEMPORARY"; then
        cleanup_atomic_temporary
        die "cannot set executable mode in $INSTALL_DIR"
    fi
    if ! mv -f -- "$ATOMIC_TEMPORARY" "$target"; then
        cleanup_atomic_temporary
        die "cannot atomically install $target"
    fi
    ATOMIC_TEMPORARY=""
    printf 'installed %s %s (SHA256 %s)\n' "$BINARY_NAME" "$VERSION" "$(file_sha256 "$target")"
}

file_sha256() {
    local path="$1"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$path" | awk '{print $1}'
    else
        shasum -a 256 "$path" | awk '{print $1}'
    fi
}

prepare_binary() {
    local temporary_root="$1"
    case "$PLATFORM" in
        linux-amd64|macos-amd64|macos-arm64|windows-amd64) ;;
        *) die "no release asset is published for $PLATFORM" ;;
    esac
    mkdir -p "$temporary_root"
    local extension=""
    [[ "$OS" == windows ]] && extension=".exe"
    local filename="${BINARY_NAME}-${PLATFORM}${extension}"
    local base_url="${RELEASE_BASE_URL}/${VERSION}"

    printf 'downloading and verifying %s from immutable tag %s\n' "$filename" "$VERSION"
    download "${base_url}/${filename}" "$temporary_root/$filename"
    download "${base_url}/SHA256SUMS" "$temporary_root/SHA256SUMS"
    download "${base_url}/SHA256SUMS.sig" "$temporary_root/SHA256SUMS.sig"
    download "${base_url}/SHA256SUMS.crt" "$temporary_root/SHA256SUMS.crt"
    download "${base_url}/${filename}.sig" "$temporary_root/${filename}.sig"
    download "${base_url}/${filename}.crt" "$temporary_root/${filename}.crt"
    verify_signature "$temporary_root" "SHA256SUMS" "$SIGNER_REPO"
    verify_checksum "$temporary_root" "$filename"
    verify_signature "$temporary_root" "$filename" "$SIGNER_REPO"
    STAGED_BINARY="$temporary_root/$filename"
}

install_from_source() {
    command -v cargo >/dev/null 2>&1 || die "Rust cargo is required for source installation"
    command -v git >/dev/null 2>&1 || die "git is required for source installation"
    detect_platform
    local source_root="$1/source"
    git clone --depth 1 --no-checkout --single-branch --no-tags \
        "https://github.com/${REPO}.git" "$source_root"
    git -C "$source_root" fetch --depth 1 origin \
        "refs/tags/$VERSION:refs/tags/$VERSION"
    git -C "$source_root" checkout --detach "refs/tags/$VERSION"
    local tag_commit
    local head_commit
    tag_commit="$(git -C "$source_root" rev-parse --verify "refs/tags/$VERSION^{commit}")" \
        || die "source tag is unavailable: $VERSION"
    head_commit="$(git -C "$source_root" rev-parse --verify HEAD)" \
        || die "source checkout has no commit"
    [[ "$tag_commit" == "$head_commit" ]] \
        || die "source checkout does not resolve the requested immutable tag: $VERSION"
    [[ -z "$(git -C "$source_root" status --porcelain)" ]] \
        || die "source checkout is unexpectedly modified"
    local cargo_root="$1/cargo-root"
    cargo install --locked --path "$source_root/tools/harness-gate" --root "$cargo_root"
    local built_binary="$cargo_root/bin/$BINARY_NAME"
    if [[ ! -f "$built_binary" && -f "${built_binary}.exe" ]]; then
        built_binary="${built_binary}.exe"
    fi
    [[ -f "$built_binary" ]] || die "cargo did not produce $BINARY_NAME"
    atomic_install "$built_binary"
    printf 'installed %s %s from immutable source tag\n' "$BINARY_NAME" "$VERSION"
}

main() {
    local from_source=0
    local with_rust=0 rust_only=0 rust_options=0
    # Core signer follows the explicit version cutover; prepare_rust
    # shadows this with the explicitly provisioned GateProbes repository.
    local SIGNER_REPO="$REPO"
    while (($# > 0)); do
        case "$1" in
            --with-rust) with_rust=1; shift ;;
            --rust-only) rust_only=1; with_rust=1; shift ;;
            --rust-root|--cache-dir|--rust-version)
                (($# >= 2)) && [[ -n "$2" ]] || die "$1 requires a value"
                rust_options=1
                case "$1" in
                    --rust-root) RUST_INSTALL_DIR="${2%/}/bin" ;;
                    --cache-dir) VERIFIER_CACHE_DIR="$2" ;;
                    --rust-version) RUST_VERSION="$2" ;;
                esac
                shift 2 ;;
            --offline)
                die "legacy bundled-runtime offline archives require their historical installer; this installer only installs standalone release binaries" ;;
            --version)
                (($# >= 2)) || die "--version requires a value"
                VERSION="$2"
                shift 2
                ;;
            --install-dir)
                (($# >= 2)) || die "--install-dir requires a value"
                INSTALL_DIR="$2"
                shift 2
                ;;
            --from-source)
                from_source=1
                shift
                ;;
            -h|--help)
                usage
                return 0
                ;;
            *)
                usage >&2
                die "unknown argument: $1"
                ;;
        esac
    done

    if (( !rust_only )); then
        [[ -n "$VERSION" ]] || { usage >&2; die "--version is required"; }
        validate_version
        select_core_repository
        SIGNER_REPO="$REPO"
    fi
    ((with_rust || !rust_options)) || die "Rust options require --with-rust or --rust-only"
    ((!rust_only || !from_source)) || die "--from-source applies to Core, not --rust-only"
    if ((with_rust)); then validate_version "v$RUST_VERSION"; fi
    INSTALL_TEMPORARY_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/harness-gate-install.XXXXXXXX")" \
        || die "cannot create temporary installation directory"
    local temporary_root="$INSTALL_TEMPORARY_ROOT"
    trap cleanup_installation EXIT
    trap 'abort_on_signal 129' HUP
    trap 'abort_on_signal 130' INT
    trap 'abort_on_signal 143' TERM
    detect_platform
    if ((with_rust || !from_source)); then ensure_cosign "$temporary_root"; fi
    # Verify the optional plugin before replacing either installed program.
    if ((with_rust)); then prepare_rust "$temporary_root/rust"; fi
    if ((!rust_only)); then
        if ((from_source)); then
            install_from_source "$temporary_root"
        else
            prepare_binary "$temporary_root/core"
            atomic_install "$STAGED_BINARY"
        fi
    fi
    if ((with_rust)); then install_rust; fi
}

main "$@"
