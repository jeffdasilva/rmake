// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! A process retains its initially validated temporary-directory spelling.
//! Relative directories remain relative when later temporary files are created.
use std::borrow::Cow;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub enum Warning {
    Environment {
        name: &'static [u8],
        value: OsString,
        error: Option<io::Error>,
    },
    Default,
}

pub struct Selection {
    directory: Cow<'static, Path>,
    warnings: Vec<Warning>,
}
impl Selection {
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn warnings(&self) -> &[Warning] {
        &self.warnings
    }
}

fn select(
    mut environment: impl FnMut(&str) -> Option<OsString>,
    mut is_directory: impl FnMut(&Path) -> io::Result<bool>,
) -> Selection {
    let mut warnings = Vec::new();
    let mut supplied = false;
    for (name, bytes) in [
        ("MAKE_TMPDIR", b"MAKE_TMPDIR".as_slice()),
        ("TMPDIR", b"TMPDIR".as_slice()),
    ] {
        let Some(value) = environment(name).filter(|value| !value.is_empty()) else {
            continue;
        };
        supplied = true;
        match is_directory(Path::new(&value)) {
            Ok(true) => {
                return Selection {
                    directory: Cow::Owned(PathBuf::from(value)),
                    warnings,
                };
            }
            status => warnings.push(Warning::Environment {
                name: bytes,
                value,
                error: status.err(),
            }),
        }
    }
    if supplied {
        warnings.push(Warning::Default);
    }
    Selection {
        directory: Cow::Borrowed(Path::new("/tmp")),
        warnings,
    }
}

pub fn initialize() -> &'static Selection {
    static SELECTED: OnceLock<Selection> = OnceLock::new();
    SELECTED.get_or_init(|| {
        select(
            |name| std::env::var_os(name),
            |path| loop {
                match std::fs::metadata(path) {
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    result => break result.map(|metadata| metadata.is_dir()),
                }
            },
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_relative_spelling_survives_and_unused_fallback_is_not_checked() {
        let selected = select(
            |name| {
                Some(
                    if name == "MAKE_TMPDIR" {
                        "relative"
                    } else {
                        "bad"
                    }
                    .into(),
                )
            },
            |name| {
                assert_eq!(name, Path::new("relative"));
                Ok(true)
            },
        );
        assert_eq!(selected.directory(), Path::new("relative"));
        assert!(selected.warnings().is_empty());
    }

    #[test]
    fn invalid_values_warn_in_order_and_empty_values_do_not_warn() {
        let selected = select(
            |name| Some(name.into()),
            |name| {
                if name == Path::new("MAKE_TMPDIR") {
                    Err(io::ErrorKind::NotFound.into())
                } else {
                    Ok(false)
                }
            },
        );
        assert_eq!(selected.directory(), Path::new("/tmp"));
        assert!(matches!(
            selected.warnings()[0],
            Warning::Environment {
                name: b"MAKE_TMPDIR",
                error: Some(_),
                ..
            }
        ));
        assert!(matches!(
            selected.warnings()[1],
            Warning::Environment {
                name: b"TMPDIR",
                error: None,
                ..
            }
        ));
        assert!(matches!(selected.warnings()[2], Warning::Default));
        assert!(
            select(
                |_| Some(OsString::new()),
                |_| panic!("empty value was examined")
            )
            .warnings()
            .is_empty()
        );
    }
}
