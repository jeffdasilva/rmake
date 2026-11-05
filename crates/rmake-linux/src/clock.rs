// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Formatting for observations already owned by the host. No filesystem I/O.
use std::ffi::{c_char, c_int, c_long};
use std::io;
use std::mem::MaybeUninit;
use std::time::{SystemTime, UNIX_EPOCH};

#[repr(C)]
struct Calendar {
    second: c_int,
    minute: c_int,
    hour: c_int,
    day: c_int,
    month: c_int,
    year: c_int,
    weekday: c_int,
    yearday: c_int,
    daylight: c_int,
    offset: c_long,
    zone: *const c_char,
}

unsafe extern "C" {
    fn time(destination: *mut c_long) -> c_long;
    fn localtime_r(seconds: *const c_long, calendar: *mut Calendar) -> *mut Calendar;
    fn ctime_r(seconds: *const c_long, buffer: *mut c_char) -> *mut c_char;
    fn strftime(
        buffer: *mut c_char,
        size: usize,
        format: *const c_char,
        calendar: *const Calendar,
    ) -> usize;
}

fn timestamp(time: SystemTime) -> io::Result<(c_long, u32)> {
    let (seconds, nanos) = match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => (i128::from(duration.as_secs()), duration.subsec_nanos()),
        Err(error) => {
            let duration = error.duration();
            let nanos = duration.subsec_nanos();
            if nanos == 0 {
                (-i128::from(duration.as_secs()), 0)
            } else {
                (-i128::from(duration.as_secs()) - 1, 1_000_000_000 - nanos)
            }
        }
    };
    c_long::try_from(seconds)
        .map(|seconds| (seconds, nanos))
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "timestamp outside libc range"))
}

/// Observe the libc wall-clock second used by the Linux reference.
pub fn current_seconds() -> io::Result<c_long> {
    // SAFETY: a NULL output pointer requests only the returned time_t.
    let seconds = unsafe { time(std::ptr::null_mut()) };
    if seconds == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(seconds)
    }
}

/// Observe the real opening or closing clock at this call, excluding ctime's newline.
pub fn current_ctime() -> io::Result<Vec<u8>> {
    let seconds = current_seconds()?;
    let mut buffer = [0_u8; 64];
    // SAFETY: libc receives one initialized time_t and a buffer larger than its
    // documented 26-byte result. A NULL return leaves no result to inspect.
    if unsafe { ctime_r(&seconds, buffer.as_mut_ptr().cast()) }.is_null() {
        return Err(io::Error::last_os_error());
    }
    let end = buffer
        .iter()
        .position(|&byte| matches!(byte, 0 | b'\n'))
        .unwrap_or(buffer.len());
    Ok(buffer[..end].to_vec())
}

/// Format a cached file timestamp without refreshing the underlying file.
pub fn file_time(time: SystemTime) -> io::Result<Vec<u8>> {
    let (seconds, nanos) = timestamp(time)?;
    let mut calendar = MaybeUninit::<Calendar>::uninit();
    // SAFETY: localtime_r initializes the supplied tm on a non-NULL return;
    // the glibc Linux tm ABI includes offset and zone after the nine integers.
    if unsafe { localtime_r(&seconds, calendar.as_mut_ptr()) }.is_null() {
        return Err(io::Error::last_os_error());
    }
    let mut buffer = [0_u8; 128];
    // SAFETY: all pointers remain valid for this call, the format is NUL
    // terminated, and the calendar was initialized by localtime_r above.
    let written = unsafe {
        strftime(
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            c"%Y-%m-%d %H:%M:%S".as_ptr(),
            calendar.as_ptr(),
        )
    };
    if written == 0 {
        return Err(io::Error::other("unable to format cached timestamp"));
    }
    let mut output = buffer[..written].to_vec();
    if nanos != 0 {
        use std::io::Write;
        write!(output, ".{nanos:09}")?;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn cached_file_times_omit_only_an_empty_fraction() {
        let whole = UNIX_EPOCH + Duration::from_secs(1_600_000_002);
        let seconds = file_time(whole).unwrap();
        assert_eq!(seconds.len(), 19);
        assert!(!seconds.contains(&b'.'));
        assert_eq!(
            file_time(whole + Duration::from_nanos(1)).unwrap(),
            [seconds.as_slice(), b".000000001"].concat()
        );
    }

    #[test]
    fn negative_fractional_times_use_floor_seconds_and_positive_nanoseconds() {
        assert_eq!(
            timestamp(UNIX_EPOCH - Duration::from_nanos(1)).unwrap(),
            (-1, 999_999_999)
        );
        assert_eq!(
            timestamp(UNIX_EPOCH - Duration::from_secs(1)).unwrap(),
            (-1, 0)
        );
        assert_eq!(
            timestamp(UNIX_EPOCH + Duration::new(7, 123)).unwrap(),
            (7, 123)
        );
    }
}
