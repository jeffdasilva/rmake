// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva
//! Safe Rust hosts must not unload code underneath a suspended native call.
use rmake_core::diagnostic::{Location, Output};
use rmake_core::native::{NativeHost, NativeLoadStatus, NativeRuntime};
use rmake_core::variables::ExpandError;
use rmake_linux::native::Runtime;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Remove(PathBuf);
impl Drop for Remove {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn compile(path: &std::path::Path, source: &[u8]) {
    let mut cc = Command::new("cc")
        .args(["-shared", "-fPIC", "-x", "c", "-o"])
        .arg(path)
        .arg("-")
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    cc.stdin.take().unwrap().write_all(source).unwrap();
    let compiled = cc.wait_with_output().unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
}
struct Host {
    target: Runtime,
    name: Vec<u8>,
    rejections: usize,
}
impl NativeHost for Host {
    fn expand(&mut self, _: &[u8]) -> Result<Vec<u8>, ExpandError> {
        Ok(b"alive".to_vec())
    }
    fn evaluate(&mut self, _: &[u8], _: Option<&Location>) -> Result<(), ExpandError> {
        let target = self.target.clone();
        let name = self.name.clone();
        let error = target.unload_with_host(&name, self).unwrap_err();
        assert!(
            error
                .diagnostic()
                .as_bytes()
                .starts_with(b"rmake: cannot unload")
        );
        self.rejections += 1;
        Ok(())
    }
    fn emit(&mut self, _: Output<'_>) -> bool {
        true
    }
}
#[test]
fn independent_runtimes_pin_active_code_and_alias_records_do_not_own_handles() {
    let temp = std::env::temp_dir().join(format!(
        "rmake-native-host-safety-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&temp).unwrap();
    let _remove = Remove(temp.clone());
    let path = temp.join("rmake_reentry_unit.so");
    let source=br#"/* Original ABI regression; no GNU header included. */
struct Place {const char *file;unsigned long line;};
extern char *gmk_expand(const char *);
extern void gmk_eval(const char *,const struct Place *);
extern void gmk_add_function(const char *,char *(*)(const char *,unsigned int,char **),unsigned int,unsigned int,unsigned int);
int plugin_is_GPL_compatible=1;
static char *probe(const char *name,unsigned int argc,char **argv){(void)name;(void)argc;(void)argv;gmk_eval("try_unload",0);return gmk_expand("VALUE");}
int rmake_reentry_unit_gmk_setup(const struct Place *location){(void)location;gmk_add_function("reentry_probe",probe,0,0,0);return 1;}
"#;
    compile(&path, source);
    let name = path.as_os_str().as_bytes().to_vec();
    let first = Runtime::new();
    let second = Runtime::new();
    let mut host = Host {
        target: first.clone(),
        name: name.clone(),
        rejections: 0,
    };
    let location = Location {
        file: b"unit.mk".as_slice().into(),
        line: 1,
    };
    first.load(&name, false, &location, &mut host).unwrap();
    second.load(&name, false, &location, &mut host).unwrap();
    let function = second.lookup(b"reentry_probe").unwrap();
    assert_eq!(
        function
            .callback
            .invoke(b"reentry_probe", &[], &mut host)
            .unwrap(),
        b"alive"
    );
    assert_eq!(host.rejections, 1);
    // The second load found a global setup symbol; it owns no second dlopen.
    assert!(second.unload_with_host(&name, &mut host).unwrap());
    assert_eq!(
        function
            .callback
            .invoke(b"reentry_probe", &[], &mut host)
            .unwrap(),
        b"alive"
    );
    assert_eq!(host.rejections, 2);
    assert!(first.unload_with_host(&name, &mut host).unwrap());
    let error = function
        .callback
        .invoke(b"reentry_probe", &[], &mut host)
        .unwrap_err();
    assert!(
        error
            .diagnostic()
            .as_bytes()
            .ends_with(b"belongs to an unloaded object")
    );
    let missing_name = temp.join("absent.so").as_os_str().as_bytes().to_vec();
    let missing = Runtime::new();
    for _ in 0..2 {
        assert_eq!(
            missing
                .load(&missing_name, false, &location, &mut host)
                .unwrap()
                .status,
            NativeLoadStatus::Failed
        );
    }
    let retry_path = temp.join("rmake_retry_unit.so");
    let retry_source=br#"/* Original required-setup retry and handle-ownership regression. */
struct Place {const char *file;unsigned long line;};
extern char *gmk_expand(const char *);
extern void gmk_add_function(const char *,char *(*)(const char *,unsigned int,char **),unsigned int,unsigned int,unsigned int);
int plugin_is_GPL_compatible=1;
static int attempts;
static char *probe(const char *name,unsigned int argc,char **argv){(void)name;(void)argc;(void)argv;return gmk_expand("VALUE");}
int rmake_retry_unit_gmk_setup(const struct Place *location){(void)location;if(attempts++==0)return 0;gmk_add_function("retry_probe",probe,0,0,0);return 1;}
"#;
    compile(&retry_path, retry_source);
    let retry_name = retry_path.as_os_str().as_bytes().to_vec();
    let retry = Runtime::new();
    assert_eq!(
        retry
            .load(&retry_name, false, &location, &mut host)
            .unwrap()
            .status,
        NativeLoadStatus::Failed
    );
    assert!(retry.lookup(b"retry_probe").is_none());
    assert_eq!(
        retry
            .load(&retry_name, false, &location, &mut host)
            .unwrap()
            .status,
        NativeLoadStatus::Loaded
    );
    let retried = retry.lookup(b"retry_probe").unwrap();
    assert_eq!(
        retried
            .callback
            .invoke(b"retry_probe", &[], &mut host)
            .unwrap(),
        b"alive"
    );
    assert!(retry.unload_with_host(&retry_name, &mut host).unwrap());
    assert!(
        retried
            .callback
            .invoke(b"retry_probe", &[], &mut host)
            .is_err()
    );

    assert!(
        std::thread::spawn(move || {
            let runtime = Runtime::new();
            let mut host = Host {
                target: runtime.clone(),
                name: name.clone(),
                rejections: 0,
            };
            let location = Location {
                file: b"other.mk".as_slice().into(),
                line: 1,
            };
            match runtime.load(&name, false, &location, &mut host) {
                Err(error) => error
                    .diagnostic()
                    .as_bytes()
                    .ends_with(b"require the process interpreter thread"),
                Ok(_) => false,
            }
        })
        .join()
        .unwrap()
    );
}
