//! Reading the definition tree that lives in the repository.
//!
//! This is the only place mtree is parsed. Everything an mtree listing can say
//! that a partition spec has no use for — ownership, modes, timestamps — is
//! rejected here rather than silently dropped, so a listing that carries it
//! fails when the archive is built and not when someone later notices the
//! metadata went missing.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use crate::{Definitions, DirEntry, DirEntryKind, Document, Error, PartitionSpec, Result};

/// Reads `inputspec/`, `machinespec/`, and `partitionspec/` under `directory`.
pub fn read(directory: &Path) -> Result<Definitions> {
    let inputs = documents(&directory.join("inputspec"))?;
    let machines = documents(&directory.join("machinespec"))?;
    if inputs.is_empty() || machines.is_empty() {
        return Err(Error::new(format!(
            "{}: no input or machine specs found",
            directory.display()
        )));
    }

    let partitions = directory.join("partitionspec");
    let mut partition_specs = Vec::new();
    for path in listings(&partitions)? {
        let reference = path
            .strip_prefix(&partitions)
            .expect("listing is under the partition directory")
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        partition_specs.push(partition_spec(&path, reference)?);
    }
    partition_specs.sort_by(|left, right| left.reference.cmp(&right.reference));

    Ok(Definitions {
        inputs,
        machines,
        partition_specs,
    })
}

fn documents(directory: &Path) -> Result<Vec<Document>> {
    let mut paths = fs::read_dir(directory)
        .map_err(|error| Error::new(format!("{}: {error}", directory.display())))?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|error| Error::new(format!("{}: {error}", directory.display())))
        })
        .collect::<Result<Vec<_>>>()?;
    paths.retain(|path| path.extension().and_then(|value| value.to_str()) == Some("toml"));
    paths.sort();

    paths
        .into_iter()
        .map(|path| {
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| Error::new(format!("{}: name is not UTF-8", path.display())))?
                .to_owned();
            let source = fs::read_to_string(&path)
                .map_err(|error| Error::new(format!("{}: {error}", path.display())))?;
            Ok(Document { name, source })
        })
        .collect()
}

fn listings(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    let mut pending = vec![directory.to_path_buf()];
    while let Some(current) = pending.pop() {
        let entries = fs::read_dir(&current)
            .map_err(|error| Error::new(format!("{}: {error}", current.display())))?;
        for entry in entries {
            let entry =
                entry.map_err(|error| Error::new(format!("{}: {error}", current.display())))?;
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|value| value.to_str()) == Some("mtree") {
                paths.push(path);
            }
        }
    }
    paths.sort();
    Ok(paths)
}

fn partition_spec(path: &Path, reference: String) -> Result<PartitionSpec> {
    use consolespec_mtree::parser::{
        PathProperty, PathType, SetProperty, Statement, UnsetProperty,
    };

    let source = fs::read_to_string(path)
        .map_err(|error| Error::new(format!("{}: {error}", path.display())))?;
    let mut input = source.as_str();
    let statements = consolespec_mtree::parser::mtree(&mut input)
        .map_err(|error| Error::new(format!("{}: {error:?}", path.display())))?;
    let mut default_kind = None;
    let mut entries = Vec::new();
    let mut paths = BTreeSet::new();

    for (line_index, statement) in statements.into_iter().enumerate() {
        let line = line_index + 1;
        match statement {
            Statement::Ignored => {}
            Statement::Set(properties) => {
                for property in properties {
                    match property {
                        SetProperty::Type(kind) => default_kind = Some(kind),
                        SetProperty::Uid(_) | SetProperty::Gid(_) | SetProperty::Mode(_) => {
                            return Err(Error::new(format!(
                                "{}:{line}: partition specs do not retain uid, gid, or mode defaults",
                                path.display()
                            )));
                        }
                    }
                }
            }
            Statement::Unset(properties) => {
                for property in properties {
                    match property {
                        UnsetProperty::Type => default_kind = None,
                        UnsetProperty::Uid | UnsetProperty::Gid | UnsetProperty::Mode => {
                            return Err(Error::new(format!(
                                "{}:{line}: partition specs do not retain uid, gid, or mode defaults",
                                path.display()
                            )));
                        }
                    }
                }
            }
            Statement::Path {
                path: entry_path,
                properties,
            } => {
                let entry_path = entry_path.to_str().ok_or_else(|| {
                    Error::new(format!(
                        "{}:{line}: mtree path is not UTF-8",
                        path.display()
                    ))
                })?;
                validate_path(path, line, entry_path)?;
                if !paths.insert(entry_path.to_owned()) {
                    return Err(Error::new(format!(
                        "{}:{line}: duplicate mtree path `{entry_path}`",
                        path.display()
                    )));
                }

                let mut kind = None;
                let mut size = None;
                let mut link = None;
                let mut md5 = None;
                let mut sha1 = None;
                let mut sha256 = None;
                for property in properties {
                    match property {
                        PathProperty::Type(value) => {
                            set_once(path, line, entry_path, "type", &mut kind, value)?;
                        }
                        PathProperty::Size(value) => {
                            set_once(path, line, entry_path, "size", &mut size, value)?;
                        }
                        PathProperty::Link(value) => {
                            let value = value.to_str().ok_or_else(|| {
                                Error::new(format!(
                                    "{}:{line}: link target is not UTF-8",
                                    path.display()
                                ))
                            })?;
                            set_once(path, line, entry_path, "link", &mut link, value.to_owned())?;
                        }
                        PathProperty::Md5Digest(value) => {
                            let value = value.inner().try_into().expect("MD5 length is fixed");
                            set_once(path, line, entry_path, "md5", &mut md5, value)?;
                        }
                        PathProperty::Sha1Digest(value) => {
                            let value = value.inner().try_into().expect("SHA-1 length is fixed");
                            set_once(path, line, entry_path, "sha1", &mut sha1, value)?;
                        }
                        PathProperty::Sha256Digest(value) => {
                            let value = value.inner().try_into().expect("SHA-256 length is fixed");
                            set_once(path, line, entry_path, "sha256", &mut sha256, value)?;
                        }
                        PathProperty::Uid(_)
                        | PathProperty::Gid(_)
                        | PathProperty::Mode(_)
                        | PathProperty::Time(_) => {
                            return Err(Error::new(format!(
                                "{}:{line}: partition spec entry `{entry_path}` contains unsupported ALPM metadata",
                                path.display()
                            )));
                        }
                    }
                }

                let kind = match kind.or(default_kind).ok_or_else(|| {
                    Error::new(format!(
                        "{}:{line}: partition spec entry `{entry_path}` has no type",
                        path.display()
                    ))
                })? {
                    PathType::Dir => DirEntryKind::Directory,
                    PathType::File => DirEntryKind::File,
                    PathType::Link => DirEntryKind::Link,
                };
                match kind {
                    DirEntryKind::Directory
                        if size.is_some()
                            || link.is_some()
                            || md5.is_some()
                            || sha1.is_some()
                            || sha256.is_some() =>
                    {
                        return Err(Error::new(format!(
                            "{}:{line}: directory `{entry_path}` has file or link metadata",
                            path.display()
                        )));
                    }
                    DirEntryKind::File if link.is_some() => {
                        return Err(Error::new(format!(
                            "{}:{line}: file `{entry_path}` has a link target",
                            path.display()
                        )));
                    }
                    DirEntryKind::Link if link.is_none() => {
                        return Err(Error::new(format!(
                            "{}:{line}: link `{entry_path}` has no target",
                            path.display()
                        )));
                    }
                    DirEntryKind::Link
                        if size.is_some()
                            || md5.is_some()
                            || sha1.is_some()
                            || sha256.is_some() =>
                    {
                        return Err(Error::new(format!(
                            "{}:{line}: link `{entry_path}` has file metadata",
                            path.display()
                        )));
                    }
                    _ => {}
                }

                entries.push(DirEntry {
                    path: entry_path.to_owned(),
                    kind,
                    size,
                    link,
                    md5,
                    sha1,
                    sha256,
                });
            }
        }
    }

    if entries.is_empty() {
        return Err(Error::new(format!(
            "{}: partition spec is empty",
            path.display()
        )));
    }
    Ok(PartitionSpec { reference, entries })
}

fn set_once<T>(
    source: &Path,
    line: usize,
    entry_path: &str,
    property: &str,
    slot: &mut Option<T>,
    value: T,
) -> Result<()> {
    if slot.replace(value).is_some() {
        return Err(Error::new(format!(
            "{}:{line}: duplicate `{property}` property for `{entry_path}`",
            source.display()
        )));
    }
    Ok(())
}

fn validate_path(source: &Path, line: usize, path: &str) -> Result<()> {
    if path == "." {
        return Ok(());
    }
    let Some(relative) = path.strip_prefix("./") else {
        return Err(Error::new(format!(
            "{}:{line}: partition path `{path}` is not relative to `.`",
            source.display()
        )));
    };
    if relative.is_empty()
        || relative
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(Error::new(format!(
            "{}:{line}: invalid partition path `{path}`",
            source.display()
        )));
    }
    Ok(())
}
