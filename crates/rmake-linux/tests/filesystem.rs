// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use rmake_core::diagnostic::FilesystemEffect;
use rmake_linux::filesystem::execute;

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "rmake-fs-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("temporary directory: {error}"),
            }
        }
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn reads_keep_raw_bytes_and_observe_each_change() {
    let directory = Directory::new();
    let path = directory.0.join(std::ffi::OsStr::from_bytes(b"raw-\xff"));
    let name = path.as_os_str().as_bytes();
    assert_eq!(execute(FilesystemEffect::ReadFile(name)), Ok(Vec::new()));
    for (data, result) in [
        (b"a\0b\xff\r\n".as_slice(), b"a\0b\xff".as_slice()),
        (b"\n\n", b"\n"),
        (b"bare\r", b"bare\r"),
        (b"", b""),
    ] {
        fs::write(&path, data).unwrap();
        assert_eq!(
            execute(FilesystemEffect::ReadFile(name)),
            Ok(result.to_vec())
        );
    }
}

#[test]
fn writes_distinguish_omitted_empty_and_existing_newline() {
    let directory = Directory::new();
    let path = directory.0.join("output");
    let name = path.as_os_str().as_bytes();
    for (text, append, result) in [
        (None, false, b"".as_slice()),
        (Some(b"".as_slice()), false, b"\n"),
        (Some(b"one".as_slice()), false, b"one\n"),
        (Some(b"two\n".as_slice()), true, b"one\ntwo\n"),
        (None, true, b"one\ntwo\n"),
        (None, false, b""),
    ] {
        assert_eq!(
            execute(FilesystemEffect::WriteFile {
                path: name,
                text,
                append
            }),
            Ok(Vec::new())
        );
        assert_eq!(fs::read(&path).unwrap(), result);
    }
}

#[test]
fn realpath_resolves_links_without_caching_observations() {
    let directory = Directory::new();
    let target = directory.0.join("target");
    let link = directory.0.join("link");
    symlink("target", &link).unwrap();
    assert_eq!(
        execute(FilesystemEffect::Realpath(link.as_os_str().as_bytes())),
        Ok(Vec::new())
    );
    fs::write(&target, b"").unwrap();
    assert_eq!(
        execute(FilesystemEffect::Realpath(link.as_os_str().as_bytes())),
        Ok(target.as_os_str().as_bytes().to_vec())
    );
    fs::remove_file(&target).unwrap();
    assert_eq!(
        execute(FilesystemEffect::Realpath(link.as_os_str().as_bytes())),
        Ok(Vec::new())
    );
}

#[test]
fn stdio_failure_stage_and_errno_survive_cleanup() {
    let directory = Directory::new();
    let name = directory.0.as_os_str().as_bytes();
    assert_eq!(
        execute(FilesystemEffect::ReadFile(name)).unwrap_err(),
        [b"read: ".as_slice(), name, b": Is a directory"].concat()
    );
    for append in [false, true] {
        assert_eq!(
            execute(FilesystemEffect::WriteFile {
                path: b"/dev/full",
                text: None,
                append
            }),
            Ok(Vec::new())
        );
        for (size, stage) in [
            (0, "close"),
            (4095, "close"),
            (4096, "write"),
            (8192, "write"),
        ] {
            let text = vec![b'x'; size];
            assert_eq!(
                execute(FilesystemEffect::WriteFile {
                    path: b"/dev/full",
                    text: Some(&text),
                    append
                })
                .unwrap_err(),
                format!("{stage}: /dev/full: No space left on device").into_bytes()
            );
        }
    }
}

#[test]
fn ffi_rejects_interior_nul_without_panicking() {
    assert!(
        execute(FilesystemEffect::ReadFile(b"bad\0name"))
            .unwrap_err()
            .ends_with(b"NUL byte in filename")
    );
    let directory = Directory::new();
    let name = directory.0.join("output");
    assert!(
        execute(FilesystemEffect::WriteFile {
            path: name.as_os_str().as_bytes(),
            text: Some(b"bad\0text"),
            append: false
        })
        .unwrap_err()
        .ends_with(b"NUL byte in file text")
    );
}
