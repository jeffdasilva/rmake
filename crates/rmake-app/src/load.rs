// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Linux load observations are scheduling effects, never persisted observations.
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Default)]
pub struct Monitor {
    proc_tried: bool,
    proc: Option<File>,
    second: u64,
    starts: u64,
    previous: u64,
    last_error: Option<i32>,
}

impl Monitor {
    fn tick(&mut self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if now > self.second {
            self.previous = if now - self.second == 1 {
                self.starts
            } else {
                0
            };
            self.starts = 0;
            self.second = now;
        }
    }

    pub fn started(&mut self) {
        self.tick();
        self.starts = self.starts.saturating_add(1);
    }

    /// Linux uses the instantaneous runnable count in /proc/loadavg, strictly
    /// greater than the limit. If unavailable, use libc's average plus recent
    /// launch pressure. A successful first job is never gated by this monitor.
    pub fn high(&mut self, limit: f64) -> (bool, Option<std::io::Error>) {
        if limit < 0.0 || limit.is_nan() {
            return (false, None);
        }
        if !self.proc_tried {
            self.proc_tried = true;
            self.proc = File::open("/proc/loadavg").ok();
        }
        if let Some(file) = &mut self.proc {
            let mut bytes = [0; 64];
            if retry(|| file.seek(SeekFrom::Start(0))).is_ok()
                && let Ok(count) = retry(|| file.read(&mut bytes))
                && let Some(running) = running_count(&bytes[..count])
            {
                return (f64::from(running) > limit, None);
            }
            // A bad proc observation permanently selects the fallback for this
            // invocation; it never freezes a stale runnable count.
            self.proc = None;
        }
        self.tick();
        match rmake_linux::process::average_load() {
            Ok(load) => (
                load + (self.starts as f64 + self.previous as f64 * 0.25) * 0.25 >= limit,
                None,
            ),
            Err(error) => {
                let code = error.raw_os_error().unwrap_or(0);
                let report = self.last_error != Some(code);
                self.last_error = Some(code);
                // Failed sampling does not deadlock the build. Recent local
                // launch pressure still applies with an unavailable average.
                (
                    (self.starts as f64 + self.previous as f64 * 0.25) * 0.25 >= limit,
                    report.then_some(error),
                )
            }
        }
    }
}

fn retry<T>(mut operation: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    loop {
        match operation() {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            result => return result,
        }
    }
}

fn running_count(bytes: &[u8]) -> Option<u32> {
    let start = bytes
        .iter()
        .enumerate()
        .filter(|(_, b)| **b == b' ')
        .nth(2)?
        .0
        + 1;
    let digits = bytes
        .get(start..)?
        .iter()
        .take_while(|b| b.is_ascii_digit());
    let mut count = None;
    for byte in digits {
        count = Some(
            count
                .unwrap_or(0_u64)
                .saturating_mul(10)
                .saturating_add(u64::from(*byte - b'0')),
        );
    }
    count.map(|value| value as u32)
}

#[cfg(test)]
mod tests {
    use super::running_count;
    #[test]
    fn proc_count_uses_instantaneous_running_jobs_and_c_unsigned_boundaries() {
        assert_eq!(running_count(b"100.0 90.0 80.0 2/450 12\n"), Some(2));
        assert_eq!(running_count(b"0 0 0 4294967296/99 1"), Some(0));
        for bad in [b"".as_slice(), b"0 0", b"0 0 0 x/99 1", b"0 0 0 -2/99 1"] {
            assert_eq!(running_count(bad), None);
        }
    }
}
