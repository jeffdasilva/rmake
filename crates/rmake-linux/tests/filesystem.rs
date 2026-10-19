// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use rmake_core::diagnostic::FilesystemEffect;
use rmake_linux::filesystem::Runtime;
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
fn wildcard_observations_share_inode_contents_and_refresh_after_effects() {
    let directory = Directory::new();
    let path = directory.0.join("d");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("old.t"), b"old").unwrap();
    symlink("d", directory.0.join("alias")).unwrap();
    let pattern = [path.as_os_str().as_bytes(), b"/*.t"].concat();
    let alias = [directory.0.as_os_str().as_bytes(), b"/alias/*.t"].concat();
    let old = path.join("old.t").as_os_str().as_bytes().to_vec();
    let new = path.join("new.t").as_os_str().as_bytes().to_vec();
    let mut runtime = Runtime::default();
    assert_eq!(
        runtime
            .execute(FilesystemEffect::Wildcard(&pattern))
            .unwrap(),
        old
    );
    fs::remove_file(path.join("old.t")).unwrap();
    fs::write(path.join("new.t"), b"new").unwrap();
    assert_eq!(
        runtime
            .execute(FilesystemEffect::Wildcard(&pattern))
            .unwrap(),
        old
    );
    assert_eq!(
        runtime.execute(FilesystemEffect::Wildcard(&alias)).unwrap(),
        directory.0.join("alias/old.t").as_os_str().as_bytes()
    );
    assert_eq!(
        runtime.execute(FilesystemEffect::Wildcard(&new)).unwrap(),
        new
    );
    runtime.execute(FilesystemEffect::ReadFile(&new)).unwrap();
    assert_eq!(
        runtime
            .execute(FilesystemEffect::Wildcard(&pattern))
            .unwrap(),
        old
    );
    runtime
        .execute(FilesystemEffect::WriteFile {
            path: &new,
            text: None,
            append: true,
        })
        .unwrap();
    assert_eq!(
        runtime
            .execute(FilesystemEffect::Wildcard(&pattern))
            .unwrap(),
        new
    );
    fs::remove_file(path.join("new.t")).unwrap();
    runtime.child_completed();
    assert!(
        runtime
            .execute(FilesystemEffect::Wildcard(&pattern))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn wildcard_cached_paths_survive_external_directory_replacement() {
    let directory = Directory::new();
    let path = directory.0.join("d");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("a"), b"a").unwrap();
    let pattern = [path.as_os_str().as_bytes(), b"/*"].concat();
    let old = path.join("a").as_os_str().as_bytes().to_vec();
    let mut runtime = Runtime::default();
    assert_eq!(
        runtime
            .execute(FilesystemEffect::Wildcard(&pattern))
            .unwrap(),
        old
    );
    fs::rename(&path, directory.0.join("old")).unwrap();
    fs::create_dir(&path).unwrap();
    fs::write(path.join("b"), b"b").unwrap();
    assert_eq!(
        runtime
            .execute(FilesystemEffect::Wildcard(&pattern))
            .unwrap(),
        old
    );
    runtime.child_completed();
    assert_eq!(
        runtime
            .execute(FilesystemEffect::Wildcard(&pattern))
            .unwrap(),
        path.join("b").as_os_str().as_bytes()
    );
}

#[test]
fn wildcard_keeps_dangling_symlinks_but_directory_suffix_follows_them() {
    let directory = Directory::new();
    let link = directory.0.join("link");
    symlink("link", &link).unwrap();
    let pattern = [directory.0.as_os_str().as_bytes(), b"/li*"].concat();
    let mut runtime = Runtime::default();
    assert_eq!(
        runtime
            .execute(FilesystemEffect::Wildcard(&pattern))
            .unwrap(),
        link.as_os_str().as_bytes()
    );
    assert!(
        runtime
            .execute(FilesystemEffect::Wildcard(
                &[pattern, b"/".to_vec()].concat()
            ))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn literal_queries_preserve_path_spelling_and_root_separator_rules() {
    let directory = Directory::new();
    fs::create_dir(directory.0.join("d")).unwrap();
    fs::write(directory.0.join("d/file"), b"original").unwrap();
    let mut runtime = Runtime::default();
    for suffix in [b"/d/file".as_slice(), b"/d//file", b"/d/../d/file"] {
        let path = [directory.0.as_os_str().as_bytes(), suffix].concat();
        assert_eq!(
            runtime.execute(FilesystemEffect::Wildcard(&path)).unwrap(),
            path
        );
    }
    // The initial Linux development/CI profile provides /tmp. GNU glob treats
    // this two-separator root prefix differently from an interior doubled '/'.
    assert_eq!(
        runtime
            .execute(FilesystemEffect::Wildcard(b"//tmp"))
            .unwrap(),
        b"/tmp"
    );
    let missing = [directory.0.as_os_str().as_bytes(), b"/missing"].concat();
    assert!(
        runtime
            .execute(FilesystemEffect::Wildcard(&missing))
            .unwrap()
            .is_empty()
    );
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
