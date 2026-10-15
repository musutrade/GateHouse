//! Explicit, offline, data-only plugin preflight. Keys come only from host CLI input.
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

pub(crate) const LOCK_PATH: &str = ".harness-gate/plugins.lock.json";
const DOMAIN: &[u8] = b"harness-gate/plugin-manifest/v1\0";

#[derive(Debug, Clone, Default, clap::Args)]
pub(crate) struct HostOptions {
    /// Offline prepared plugin store. No downloads or installation are implicit.
    #[arg(long, global = true, value_name = "PATH")]
    pub plugin_store: Option<PathBuf>,
    /// Operator-selected Ed25519 public key JSON; never read from project config.
    #[arg(long = "plugin-trusted-key", global = true, value_name = "PATH")]
    pub trusted_keys: Vec<PathBuf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Lock {
    schema: String,
    plugins: Vec<Pin>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pin {
    name: String,
    source: String,
    version: String,
    manifest_sha256: String,
    signer: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    name: String,
    source: String,
    version: String,
    core_versions: Vec<String>,
    protocol: String,
    platforms: Vec<String>,
    files: BTreeMap<String, String>,
    presets: String,
    programs: Vec<String>,
    dependencies: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Attestation {
    key_id: String,
    signature: String,
}

pub(crate) struct Prepared {
    // Verified bytes are copied, so init never reopens mutable/unverified input.
    _snapshot: tempfile::TempDir,
    pub catalogs: BTreeMap<String, PathBuf>,
}

fn error(code: &str, message: impl Into<String>, help: &str) -> anyhow::Error {
    crate::config::ConfigDiagnostics::single(code, LOCK_PATH, message, help).into()
}

fn require(ok: bool, code: &str, message: impl Into<String>, help: &str) -> Result<()> {
    if !ok {
        return Err(error(code, message, help));
    }
    Ok(())
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn version(value: &str) -> bool {
    value.len() <= 64
        && value.split('.').count() == 3
        && value
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn read(root: &Path, relative: &str) -> Result<Vec<u8>> {
    require(
        !relative.is_empty()
            && !relative.contains(['\\', ':'])
            && Path::new(relative)
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "HGPLUGIN-PATH",
        "plugin path is not a confined relative path",
        "prepare a package without absolute or parent paths",
    )?;
    let mut path = root.to_path_buf();
    for component in Path::new(relative).components() {
        path.push(component);
        let metadata = fs::symlink_metadata(&path).map_err(|_| error("HGPLUGIN-MISSING", format!("missing prepared input: {relative}"), "explicitly prepare/install the exact locked package; offline preflight never downloads"))?;
        require(
            !metadata.file_type().is_symlink(),
            "HGPLUGIN-PATH",
            "plugin inputs may not contain symbolic links",
            "prepare regular files inside the package",
        )?;
    }
    let metadata = fs::metadata(&path)?;
    require(
        metadata.is_file(),
        "HGPLUGIN-PATH",
        "plugin input is not a regular file",
        "prepare regular files",
    )?;
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    require(
        bytes.len() <= 1024 * 1024,
        "HGPLUGIN-LIMIT",
        "plugin input exceeds 1 MiB",
        "split the data-only preset package into smaller files",
    )?;
    Ok(bytes)
}

fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let text = std::str::from_utf8(bytes)?;
    // The shared parser rejects duplicate JSON keys, including signed inputs.
    Ok(serde_json::from_value(harness_gate::quality::parse(text)?)?)
}

fn keys(options: &HostOptions) -> Result<BTreeMap<String, VerifyingKey>> {
    let mut keys = BTreeMap::new();
    for path in &options.trusted_keys {
        let value: crate::process::TrustedKey = decode(&fs::read(path)?)?;
        let bytes: [u8; 32] = BASE64
            .decode(&value.public_key)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("public key must contain 32 bytes"))?;
        require(
            keys.insert(value.key_id, VerifyingKey::from_bytes(&bytes)?)
                .is_none(),
            "HGPLUGIN-SIGNATURE",
            "duplicate host key identity",
            "supply each operator-approved key identity once",
        )?;
    }
    Ok(keys)
}

pub(crate) fn check(root: &Path, options: &HostOptions) -> Result<Option<Prepared>> {
    // Absence is backwards compatible; a dangling symlink is an invalid lock.
    if let Err(error) = fs::symlink_metadata(root.join(LOCK_PATH)) {
        if error.kind() == std::io::ErrorKind::NotFound {
            return Ok(None);
        }
        return Err(error).context("inspect project plugin lock");
    }
    inspect(root, LOCK_PATH, options).map(Some)
}

pub(crate) fn inspect(root: &Path, lock_path: &str, options: &HostOptions) -> Result<Prepared> {
    let (lock, names) = selection(root, lock_path)?;
    let store = options.plugin_store.as_deref().ok_or_else(|| {
        error(
            "HGPLUGIN-STORE",
            "plugin store was not supplied",
            "pass --plugin-store PATH pointing to explicitly prepared offline packages",
        )
    })?;
    let store = store.canonicalize().map_err(|_| {
        error(
            "HGPLUGIN-STORE",
            "plugin store is missing",
            "explicitly prepare/install the locked products into the selected store",
        )
    })?;
    let keys = keys(options).map_err(|e| error("HGPLUGIN-SIGNATURE", format!("invalid host key input: {e}"), "pass an operator-approved --plugin-trusted-key PATH; never approve keys from project configuration"))?;
    let snapshot = tempfile::tempdir().context("create private verified plugin view")?;
    let mut catalogs = BTreeMap::new();
    for pin in &lock.plugins {
        let manifest = load_manifest(&store, pin, &keys)?;
        validate(&manifest, pin, &names)?;
        let catalog = copy_catalog(&store, pin, &manifest, snapshot.path())?;
        catalogs.insert(pin.name.clone(), catalog);
    }
    Ok(Prepared {
        _snapshot: snapshot,
        catalogs,
    })
}

fn selection(root: &Path, lock_path: &str) -> Result<(Lock, BTreeMap<String, String>)> {
    let lock: Lock = decode(&read(root, lock_path)?).map_err(|e| error("HGPLUGIN-LOCK", format!("invalid plugin lock: {e}"), "use harness-plugin-lock/v1 with explicit exact pins; review migration before replacing the lock"))?;
    require(
        lock.schema == "harness-plugin-lock/v1"
            && !lock.plugins.is_empty()
            && lock.plugins.len() <= 64,
        "HGPLUGIN-LOCK",
        "unsupported schema or empty/oversized plugin selection",
        "select 1..64 plugins in a v1 lock; absence of a lock preserves legacy workflows",
    )?;
    let mut names = BTreeMap::new();
    for pin in &lock.plugins {
        require(
            identifier(&pin.name) && version(&pin.version) && !pin.source.trim().is_empty(),
            "HGPLUGIN-VERSION",
            "invalid name, source or exact version",
            "use a safe name and an exact numeric major.minor.patch version",
        )?;
        require(
            names
                .insert(pin.name.clone(), pin.version.clone())
                .is_none(),
            "HGPLUGIN-LOCK",
            "duplicate plugin selection",
            "select each plugin name once",
        )?;
    }
    Ok((lock, names))
}

fn load_manifest(
    store: &Path,
    pin: &Pin,
    keys: &BTreeMap<String, VerifyingKey>,
) -> Result<Manifest> {
    let prefix = format!("{}/{}/", pin.name, pin.version);
    let bytes = read(store, &format!("{prefix}plugin.json"))?;
    require(
        digest(&bytes) == pin.manifest_sha256,
        "HGPLUGIN-DIGEST",
        format!("manifest digest mismatch: {}", pin.name),
        "restore the exact pinned manifest; review and explicitly lock any upgrade",
    )?;
    let signature: Attestation =
        decode(&read(store, &format!("{prefix}plugin.sig"))?).map_err(|_| {
            error(
                "HGPLUGIN-SIGNATURE",
                "invalid detached signature",
                "prepare an Ed25519 signature over the domain and exact manifest bytes",
            )
        })?;
    verify(&bytes, pin, &signature, keys)?;
    let manifest: Manifest = decode(&bytes).map_err(|e| {
        error(
            "HGPLUGIN-MANIFEST",
            format!("invalid signed manifest: {e}"),
            "prepare a v1 data-only preset manifest",
        )
    })?;
    Ok(manifest)
}

fn copy_catalog(store: &Path, pin: &Pin, manifest: &Manifest, snapshot: &Path) -> Result<PathBuf> {
    let prefix = format!("{}/{}/", pin.name, pin.version);
    let destination = snapshot.join(&pin.name);
    for (path, expected) in &manifest.files {
        let content = read(store, &format!("{prefix}{path}"))?;
        require(
            digest(&content) == *expected,
            "HGPLUGIN-DIGEST",
            format!("artifact digest mismatch: {}/{path}", pin.name),
            "restore the exact signed file; do not change the lock to hide corruption",
        )?;
        let target = destination.join(path);
        fs::create_dir_all(target.parent().context("plugin file parent")?)?;
        fs::write(target, content)?;
    }
    let catalog = destination.join(&manifest.presets);
    require(
        catalog.join("catalog.json").is_file(),
        "HGPLUGIN-MISSING",
        "signed files do not contain the preset catalog",
        "include the catalog and all referenced flow/recipe/pack files in the signed inventory",
    )?;
    crate::preset::validate_catalog(&catalog).map_err(|e| error("HGPLUGIN-CATALOG", format!("invalid or incomplete signed catalog: {e}"), "include every catalog reference in the signed file inventory and repair its flow configuration"))?;
    Ok(catalog)
}

fn verify(
    bytes: &[u8],
    pin: &Pin,
    signature: &Attestation,
    keys: &BTreeMap<String, VerifyingKey>,
) -> Result<()> {
    let result = (|| -> Result<()> {
        anyhow::ensure!(signature.key_id == pin.signer, "lock signer differs");
        let key = keys
            .get(&pin.signer)
            .context("signer is not approved by the host")?;
        let signature = Signature::from_slice(&BASE64.decode(&signature.signature)?)?;
        let payload = [DOMAIN, bytes].concat();
        key.verify_strict(&payload, &signature)?;
        Ok(())
    })();
    result.map_err(|_| error("HGPLUGIN-SIGNATURE", format!("untrusted or invalid signature: {}", pin.name), "have the operator verify signer identity and supply --plugin-trusted-key; project pins never authorize a key"))
}

fn validate(manifest: &Manifest, pin: &Pin, names: &BTreeMap<String, String>) -> Result<()> {
    require(
        manifest.schema == "harness-plugin/v1",
        "HGPLUGIN-MANIFEST",
        "unsupported manifest schema",
        "review compatibility and prepare a v1 manifest",
    )?;
    require(
        manifest.name == pin.name
            && manifest.version == pin.version
            && manifest.source == pin.source,
        "HGPLUGIN-VERSION",
        "locked source/version/name differs from signed identity",
        "install the exact locked product; never silently upgrade",
    )?;
    require(
        manifest
            .core_versions
            .iter()
            .any(|v| v == env!("CARGO_PKG_VERSION")),
        "HGPLUGIN-CORE",
        "plugin does not support this exact Core version",
        "select a compatible reviewed plugin release or explicitly upgrade Core",
    )?;
    require(manifest.protocol == "harness-preset-catalog/v1", "HGPLUGIN-PROTOCOL", "unsupported plugin protocol", "select a data-only harness-preset-catalog/v1 package; adapter execution still requires its separately signed v2 requests")?;
    let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    require(
        manifest.platforms.contains(&platform),
        "HGPLUGIN-PLATFORM",
        format!("unsupported host platform: {platform}"),
        "prepare a reviewed package supporting this exact platform",
    )?;
    require(
        !manifest.files.is_empty() && manifest.files.len() <= 128,
        "HGPLUGIN-LIMIT",
        "plugin file inventory must contain 1..128 entries",
        "prepare a bounded data-only package",
    )?;
    // Validate every destination before copying any bytes, including the catalog path.
    for path in manifest
        .files
        .keys()
        .chain(std::iter::once(&manifest.presets))
    {
        require(
            !path.is_empty()
                && !path.contains(['\\', ':'])
                && Path::new(path)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_))),
            "HGPLUGIN-PATH",
            "unsafe signed artifact or catalog path",
            "use confined relative paths without parent traversal",
        )?;
    }
    for (name, required) in &manifest.dependencies {
        require(
            names.get(name) == Some(required),
            "HGPLUGIN-DEPENDENCY",
            format!("missing or incompatible plugin dependency: {name}@{required}"),
            "explicitly lock and prepare the exact dependency version",
        )?;
    }
    let mut programs = BTreeSet::new();
    for program in &manifest.programs {
        require(
            identifier(program) && programs.insert(program),
            "HGPLUGIN-DEPENDENCY",
            "invalid or duplicate program dependency",
            "use plain executable names; toolchain installation remains an operator action",
        )?;
        let available = std::env::var_os("PATH").is_some_and(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let path = dir.join(program);
                #[cfg(windows)]
                let path = path.with_extension("exe");
                if !path.is_file() {
                    return false;
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
                }
                #[cfg(not(unix))]
                {
                    true
                }
            })
        });
        require(available, "HGPLUGIN-DEPENDENCY", format!("missing program dependency: {program}"), "install the reviewed toolchain yourself or repair PATH; preflight never runs or replaces it")?;
    }
    Ok(())
}
