// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Percentage diagnostics follow the current libc locale and floating state.
//! The adapter performs no Make expansion, file search, or descriptor output.
use std::ffi::{c_char, c_int, c_uint};
use std::io::{self, Write};
use std::ptr;

unsafe extern "C" {
    fn rm_database_percent(
        scratch: *mut c_char,
        capacity: usize,
        owned: *mut *mut c_char,
        numerator: u64,
        denominator: u64,
        fractional_digits: c_uint,
    ) -> c_int;
    fn rm_database_percent_free(owned: *mut c_char);
}

struct Owned(*mut c_char);
impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { rm_database_percent_free(self.0) };
        }
    }
}

pub struct Formatting;
impl rmake_core::database::NumericFormatting for Formatting {
    fn write_percent(
        &self,
        output: &mut dyn Write,
        numerator: u64,
        denominator: u64,
        fractional_digits: u8,
    ) -> io::Result<()> {
        let mut scratch = [0_u8; 128];
        let mut owned = ptr::null_mut();
        // The helper allocates only if a locale's decimal point exceeds the
        // conservative stack bound. It returns exactly the initialized length.
        let length = unsafe {
            rm_database_percent(
                scratch.as_mut_ptr().cast(),
                scratch.len(),
                &mut owned,
                numerator,
                denominator,
                fractional_digits.into(),
            )
        };
        if length < 0 {
            return Err(io::Error::last_os_error());
        }
        let owned = Owned(owned);
        let pointer = if owned.0.is_null() {
            scratch.as_ptr()
        } else {
            owned.0.cast()
        };
        // C checks the snprintf result against the selected buffer capacity.
        let bytes = unsafe { std::slice::from_raw_parts(pointer, length as usize) };
        output.write_all(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmake_core::database::NumericFormatting;

    fn percentage(numerator: u64, denominator: u64, digits: u8) -> Vec<u8> {
        let mut result = Vec::new();
        Formatting
            .write_percent(&mut result, numerator, denominator, digits)
            .unwrap();
        result
    }

    #[test]
    fn integer_inputs_zero_denominators_and_precision_errors_are_explicit() {
        assert_eq!(percentage(41, 1024, 0), b"4");
        assert_eq!(percentage(5, 92, 1), b"5.4");
        assert_eq!(percentage(0, 19, 0), b"0");
        assert_eq!(percentage(19, 0, 0), b"0");
        let mut result = Vec::new();
        assert_eq!(
            Formatting
                .write_percent(&mut result, 1, 3, 2)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert!(result.is_empty());
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn native_rounding_is_live_and_positive_zero_stays_positive() {
        unsafe extern "C" {
            fn fegetround() -> c_int;
            fn fesetround(mode: c_int) -> c_int;
        }
        struct Restore(c_int);
        impl Drop for Restore {
            fn drop(&mut self) {
                assert_eq!(unsafe { fesetround(self.0) }, 0);
            }
        }
        let _restore = Restore(unsafe { fegetround() });
        // Public glibc/x86-64 FE_UPWARD and FE_DOWNWARD encodings. Each Rust
        // test has its own OS thread; restore even when an assertion unwinds.
        assert_eq!(unsafe { fesetround(0x800) }, 0);
        assert_eq!(percentage(41, 1024, 0), b"5");
        assert_eq!(percentage(5, 92, 1), b"5.5");
        assert_eq!(unsafe { fesetround(0x400) }, 0);
        assert_eq!(percentage(0, 19, 0), b"0");
        assert_eq!(percentage(130, 1636, 0), b"7");
    }
}
