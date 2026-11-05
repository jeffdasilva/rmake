// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Linux load observations are scheduling effects, never persisted observations.
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::os::raw::c_long;

pub enum Observation<'a> {
    Method { procfs: bool },
    Running(u32),
    Malformed(&'a [u8]),
    ReadFailure(&'a io::Error),
    AverageFailure(&'a io::Error),
    Estimate { estimated: f64, actual: f64 },
}

#[derive(Default)]
pub struct Monitor {
    proc_tried: bool,
    proc: Option<File>,
    second: c_long,
    starts: u64,
    previous: f64,
    last_error: Option<i32>,
}

impl Monitor {
    fn tick(&mut self, now: c_long) {
        if now > self.second {
            self.previous = if now - self.second == 1 {
                self.starts as f64 * 0.25
            } else {
                0.0
            };
            self.starts = 0;
            self.second = now;
        }
    }

    pub fn started(&mut self) {
        self.starts = self.starts.saturating_add(1);
    }

    pub fn reaped(&mut self) {
        self.starts = self.starts.saturating_sub(1);
    }

    /// Use the instantaneous runnable count when available. The fallback
    /// advances its pressure epoch at a sample, independently of job launches.
    pub fn high(&mut self, limit: f64, mut observe: impl FnMut(Observation<'_>)) -> bool {
        if limit < 0.0 || limit.is_nan() {
            return false;
        }
        if !self.proc_tried {
            self.proc_tried = true;
            self.proc = File::open("/proc/loadavg").ok();
            observe(Observation::Method {
                procfs: self.proc.is_some(),
            });
        }
        if let Some(file) = &mut self.proc {
            let mut bytes = [0; 64];
            match retry(|| file.seek(SeekFrom::Start(0)))
                .and_then(|_| retry(|| file.read(&mut bytes)))
            {
                Ok(count) => {
                    let sample = &bytes[..count];
                    if let Some(running) = running_count(sample) {
                        observe(Observation::Running(running));
                        return f64::from(running) > limit;
                    }
                    observe(Observation::Malformed(sample));
                }
                Err(error) => observe(Observation::ReadFailure(&error)),
            }
            self.proc = None;
        }
        let actual = match rmake_linux::process::average_load() {
            Ok(load) => load,
            Err(error) => {
                let code = error.raw_os_error().unwrap_or(0);
                if self.last_error != Some(code) {
                    observe(Observation::AverageFailure(&error));
                    self.last_error = Some(code);
                }
                0.0
            }
        };
        if let Ok(now) = rmake_linux::clock::current_seconds() {
            self.tick(now);
        }
        let estimated = actual + (self.starts as f64 + self.previous) * 0.25;
        observe(Observation::Estimate { estimated, actual });
        estimated >= limit
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
    use super::{Monitor, running_count};

    #[test]
    fn fallback_pressure_epochs_change_on_sampling_and_count_live_reaps() {
        let mut monitor = Monitor::default();
        monitor.started();
        monitor.tick(100);
        assert_eq!((monitor.starts, monitor.previous), (0, 0.0));

        monitor.started();
        monitor.started();
        monitor.reaped();
        monitor.tick(100);
        assert_eq!((monitor.starts, monitor.previous), (1, 0.0));
        monitor.tick(101);
        assert_eq!((monitor.starts, monitor.previous), (0, 0.25));

        monitor.reaped();
        monitor.started();
        monitor.tick(104);
        assert_eq!((monitor.starts, monitor.previous), (0, 0.0));
    }
    #[test]
    fn proc_count_uses_instantaneous_running_jobs_and_c_unsigned_boundaries() {
        assert_eq!(running_count(b"100.0 90.0 80.0 2/450 12\n"), Some(2));
        assert_eq!(running_count(b"0 0 0 4294967296/99 1"), Some(0));
        for bad in [b"".as_slice(), b"0 0", b"0 0 0 x/99 1", b"0 0 0 -2/99 1"] {
            assert_eq!(running_count(bad), None);
        }
    }
}
