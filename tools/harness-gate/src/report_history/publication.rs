//! Complete writes privately, then publish without replacing existing paths.
use anyhow::{ensure, Context, Result};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

const FILES: [&str; 2] = ["history.html", "provenance.json"];

pub(super) fn publish(destination: &Path, html: &[u8], provenance: &[u8]) -> Result<()> {
    publish_with(destination, [html, provenance], write, |source, target| {
        fs::hard_link(source, target)
    })
}
fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        left.dev() == right.dev() && left.ino() == right.ino()
    }
    #[cfg(not(unix))]
    {
        matches!((left.created(), right.created(), left.modified(), right.modified()),
            (Ok(a), Ok(b), Ok(c), Ok(d)) if a == b && c == d && left.len() == right.len())
    }
}
fn rollback(
    destination: &Path,
    staging: &Path,
    published: usize,
    contents: [&[u8]; 2],
) -> Result<()> {
    for (name, expected) in FILES.iter().zip(contents).take(published) {
        let path = destination.join(name);
        let meta = fs::symlink_metadata(&path)?;
        ensure!(
            meta.is_file()
                && !meta.file_type().is_symlink()
                && same_file(&meta, &fs::metadata(staging.join(name))?),
            "history publication conflict; preserving {}",
            path.display()
        );
        let mut bytes = Vec::new();
        fs::File::open(&path)?
            .take(expected.len() as u64 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes == expected,
            "history content changed; preserving {}",
            path.display()
        );
        fs::remove_file(path)?;
    }
    fs::remove_dir(destination)
        .context("history output has conflicting contents; preserving directory")
}
fn publish_with(
    destination: &Path,
    contents: [&[u8]; 2],
    mut writer: impl FnMut(&Path, &[u8]) -> std::io::Result<()>,
    mut linker: impl FnMut(&Path, &Path) -> std::io::Result<()>,
) -> Result<()> {
    let staging = tempfile::Builder::new()
        .prefix(".history-")
        .tempdir_in(destination.parent().context("history output parent")?)?;
    for (name, bytes) in FILES.iter().zip(contents) {
        writer(&staging.path().join(name), bytes)?;
    }
    fs::create_dir(destination).context("history output must be a new directory")?;
    for (published, name) in FILES.iter().enumerate() {
        if let Err(error) = linker(&staging.path().join(name), &destination.join(name)) {
            rollback(destination, staging.path(), published, contents).with_context(|| {
                format!(
                    "publication failed ({error}); cleanup incomplete at {}",
                    destination.display()
                )
            })?;
            return Err(error)
                .context("publish history without replacement (filesystem hard links required)");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_second_write_or_link_does_not_leave_published_output() {
        for fail_write in [true, false] {
            let root = tempfile::tempdir().unwrap();
            let output = root.path().join("output");
            let mut writes = 0;
            let mut links = 0;
            assert!(publish_with(
                &output,
                [b"html", b"provenance"],
                |path, bytes| {
                    writes += 1;
                    if fail_write && writes == 2 {
                        return Err(std::io::Error::other("injected write failure"));
                    }
                    write(path, bytes)
                },
                |source, target| {
                    links += 1;
                    if !fail_write && links == 2 {
                        return Err(std::io::Error::other("injected link failure"));
                    }
                    fs::hard_link(source, target)
                }
            )
            .is_err());
            assert!(!output.exists());
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
        }
    }
    #[test]
    fn externally_modified_published_content_is_preserved() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("output");
        let mut links = 0;
        let error = publish_with(
            &output,
            [b"html", b"provenance"],
            write,
            |source, target| {
                links += 1;
                if links == 2 {
                    fs::write(output.join("history.html"), b"external edit")?;
                    return Err(std::io::Error::other("injected link failure"));
                }
                fs::hard_link(source, target)
            },
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("content changed"));
        assert_eq!(
            fs::read(output.join("history.html")).unwrap(),
            b"external edit"
        );
    }
    #[test]
    fn late_foreign_content_is_preserved_and_reported() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("output");
        let mut links = 0;
        let error = publish_with(
            &output,
            [b"html", b"provenance"],
            write,
            |source, target| {
                links += 1;
                if links == 2 {
                    fs::write(target, b"foreign")?;
                }
                fs::hard_link(source, target)
            },
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("cleanup incomplete"));
        assert_eq!(
            fs::read(output.join("provenance.json")).unwrap(),
            b"foreign"
        );
        assert!(!output.join("history.html").exists());
    }
}
