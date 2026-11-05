// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! GNU usage diagnostics for rejected startup argv sources.

use rmake_core::messages::{Argument, checked_format};
use std::ffi::CStr;

// Declarative 4.4.1 display keys, observed at the verified GNU binary and
// matched to its attributed message catalogs. See docs/startup-usage.md.
const OPTIONS: &[&CStr] = &[
    c"Options:\n",
    c"  -b, -m                      Ignored for compatibility.\n",
    c"  -B, --always-make           Unconditionally make all targets.\n",
    c"  -C DIRECTORY, --directory=DIRECTORY\n                              Change to DIRECTORY before doing anything.\n",
    c"  -d                          Print lots of debugging information.\n",
    c"  --debug[=FLAGS]             Print various types of debugging information.\n",
    c"  -e, --environment-overrides\n                              Environment variables override makefiles.\n",
    c"  -E STRING, --eval=STRING    Evaluate STRING as a makefile statement.\n",
    c"  -f FILE, --file=FILE, --makefile=FILE\n                              Read FILE as a makefile.\n",
    c"  -h, --help                  Print this message and exit.\n",
    c"  -i, --ignore-errors         Ignore errors from recipes.\n",
    c"  -I DIRECTORY, --include-dir=DIRECTORY\n                              Search DIRECTORY for included makefiles.\n",
    c"  -j [N], --jobs[=N]          Allow N jobs at once; infinite jobs with no arg.\n",
    c"  --jobserver-style=STYLE     Select the style of jobserver to use.\n",
    c"  -k, --keep-going            Keep going when some targets can't be made.\n",
    c"  -l [N], --load-average[=N], --max-load[=N]\n                              Don't start multiple jobs unless load is below N.\n",
    c"  -L, --check-symlink-times   Use the latest mtime between symlinks and target.\n",
    c"  -n, --just-print, --dry-run, --recon\n                              Don't actually run any recipe; just print them.\n",
    c"  -o FILE, --old-file=FILE, --assume-old=FILE\n                              Consider FILE to be very old and don't remake it.\n",
    c"  -O[TYPE], --output-sync[=TYPE]\n                              Synchronize output of parallel jobs by TYPE.\n",
    c"  -p, --print-data-base       Print make's internal database.\n",
    c"  -q, --question              Run no recipe; exit status says if up to date.\n",
    c"  -r, --no-builtin-rules      Disable the built-in implicit rules.\n",
    c"  -R, --no-builtin-variables  Disable the built-in variable settings.\n",
    c"  --shuffle[={SEED|random|reverse|none}]\n                              Perform shuffle of prerequisites and goals.\n",
    c"  -s, --silent, --quiet       Don't echo recipes.\n",
    c"  --no-silent                 Echo recipes (disable --silent mode).\n",
    c"  -S, --no-keep-going, --stop\n                              Turns off -k.\n",
    c"  -t, --touch                 Touch targets instead of remaking them.\n",
    c"  --trace                     Print tracing information.\n",
    c"  -v, --version               Print the version number of make and exit.\n",
    c"  -w, --print-directory       Print the current directory.\n",
    c"  --no-print-directory        Turn off -w, even if it was turned on implicitly.\n",
    c"  -W FILE, --what-if=FILE, --new-file=FILE, --assume-new=FILE\n                              Consider FILE to be infinitely new.\n",
    c"  --warn-undefined-variables  Warn when an undefined variable is referenced.\n",
];

fn translate(key: &'static CStr) -> Vec<u8> {
    rmake_linux::messages::lookup_static(key).unwrap_or_else(|_| key.to_bytes().to_vec())
}

fn format(key: &'static CStr, arguments: &[Argument<'_>]) -> Vec<u8> {
    checked_format(&translate(key), arguments)
        .or_else(|_| checked_format(key.to_bytes(), arguments))
        .expect("typed built-in usage format")
}

pub(crate) fn getopt(key: &'static CStr, arguments: &[Argument<'_>], localized: bool) -> Vec<u8> {
    if localized
        && let Ok(template) = rmake_linux::messages::lookup_libc(key)
        && let Ok(message) = checked_format(&template, arguments)
    {
        return message;
    }
    checked_format(key.to_bytes(), arguments).expect("typed libc getopt format")
}

pub(crate) fn empty(option: &[u8]) -> Vec<u8> {
    let prefix = if option.starts_with(b"--") { 2 } else { 1 };
    format(
        c"the '%s%s' option requires a non-empty string argument",
        &[
            Argument::Bytes(&option[..prefix]),
            Argument::Bytes(&option[prefix..]),
        ],
    )
}

pub(crate) fn jobs() -> Vec<u8> {
    format(
        c"the '-%c' option requires a positive integer argument",
        &[Argument::Character(b'j')],
    )
}

pub(crate) fn output_sync_mode(mode: &[u8]) -> Vec<u8> {
    format(c"unknown output-sync type '%s'", &[Argument::Bytes(mode)])
}

pub(crate) fn invalid_sync_mutex(authentication: &[u8]) -> Vec<u8> {
    format(
        c"invalid --sync-mutex string '%s'",
        &[Argument::Bytes(authentication)],
    )
}

pub(crate) fn sync_mutex_open(authentication: &[u8], error: &std::io::Error) -> Vec<u8> {
    let path = authentication
        .strip_prefix(b"fnm:")
        .unwrap_or(authentication);
    let detail = error
        .raw_os_error()
        .and_then(rmake_linux::messages::errno_detail)
        .unwrap_or_else(|| crate::engine::os_error(error).into_bytes());
    format(
        c"cannot open output sync mutex %s: %s",
        &[Argument::Bytes(path), Argument::Bytes(&detail)],
    )
}

pub(crate) fn jobserver_style(style: &[u8]) -> Vec<u8> {
    format(
        c"unknown jobserver auth style '%s'",
        &[Argument::Bytes(style)],
    )
}

pub(crate) fn text(program: &[u8]) -> Vec<u8> {
    let mut output = format(
        c"Usage: %s [options] [target] ...\n",
        &[Argument::Bytes(program)],
    );
    for key in OPTIONS {
        output.extend_from_slice(&translate(key));
    }
    output.extend_from_slice(&format(
        c"\nThis program built for %s\n",
        &[Argument::Bytes(b"x86_64-pc-linux-gnu")],
    ));
    output.extend_from_slice(&translate(c"Report bugs to <bug-make@gnu.org>\n"));
    output
}
