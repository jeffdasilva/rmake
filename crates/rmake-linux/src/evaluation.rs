// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Same-thread parser reentry with bounded additional stack segments.
use rmake_core::diagnostic::EvaluationStack;
use rmake_core::parser::ParseError;
use std::cell::Cell;

const RED_ZONE: usize = 1024 * 1024;
const SEGMENT: usize = 4 * 1024 * 1024;
const EXTRA_LIMIT: usize = 64 * 1024 * 1024;
thread_local! { static EXTRA: Cell<usize> = const { Cell::new(0) }; }
struct Adapter;
static ADAPTER: Adapter = Adapter;
struct Reservation;
impl Drop for Reservation {
    fn drop(&mut self) {
        EXTRA.with(|used| used.set(used.get() - SEGMENT));
    }
}

/// Stack switching is enabled only for the qualified x86-64 Linux profile.
/// Other library profiles retain the core's native recursion guard.
pub fn adapter() -> Option<&'static dyn EvaluationStack> {
    cfg!(target_arch = "x86_64").then_some(&ADAPTER)
}
impl EvaluationStack for Adapter {
    fn run(
        &self,
        action: &mut dyn FnMut() -> Result<(), ParseError>,
    ) -> Option<Result<(), ParseError>> {
        let available = stacker::remaining_stack()?;
        if available >= RED_ZONE {
            return Some(action());
        }
        let reserved = EXTRA.with(|used| {
            if used.get() >= EXTRA_LIMIT {
                false
            } else {
                used.set(used.get() + SEGMENT);
                true
            }
        });
        if !reserved {
            return None;
        }
        let _reservation = Reservation;
        Some(stacker::grow(SEGMENT, action))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn small_stack_keeps_thread_identity_and_executes_once() {
        std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(|| {
                let identity = std::thread::current().id();
                let mut calls = 0;
                ADAPTER
                    .run(&mut || {
                        calls += 1;
                        assert_eq!(std::thread::current().id(), identity);
                        assert!(stacker::remaining_stack().unwrap() > RED_ZONE);
                        Ok(())
                    })
                    .unwrap()
                    .unwrap();
                assert_eq!(calls, 1);
                assert_eq!(EXTRA.with(Cell::get), 0);
            })
            .unwrap()
            .join()
            .unwrap();
    }
    #[test]
    fn exhaustion_rejects_before_callback_and_unwinding_releases_storage() {
        std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(|| {
                EXTRA.with(|used| used.set(EXTRA_LIMIT));
                assert!(ADAPTER.run(&mut || panic!("must not execute")).is_none());
                EXTRA.with(|used| used.set(0));
                let result = std::panic::catch_unwind(|| {
                    ADAPTER.run(&mut || panic!("test unwind"));
                });
                assert!(result.is_err());
                assert_eq!(EXTRA.with(Cell::get), 0);
                ADAPTER.run(&mut || Ok(())).unwrap().unwrap();
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
