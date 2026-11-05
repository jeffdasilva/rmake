// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Process debug selection has one signal-safe word and no borrowed handler data.
use std::io;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

const MASK: usize = 0xff;
const ENABLED: usize = 0x100;
const LOW: usize = MASK | ENABLED;
static NEXT: AtomicUsize = AtomicUsize::new(1);
static STATE: AtomicUsize = AtomicUsize::new(0);
static PREVIOUS: AtomicUsize = AtomicUsize::new(usize::MAX);

pub(super) struct Owner {
    token: usize,
    // Installation and restoration stay on their owning Rust thread. Foreign
    // threads may deliver signals; the handler reads only the static word.
    _thread: PhantomData<Rc<()>>,
}

/// A process-controller binding; unit-only controllers never receive one.
#[derive(Clone, Copy)]
pub struct Handle {
    token: usize,
    _thread: PhantomData<Rc<()>>,
}

fn toggled(state: usize) -> Option<usize> {
    (state & ENABLED != 0).then(|| (state & !MASK) | usize::from(state & MASK == 0))
}

extern "C" fn caught(_: std::ffi::c_int) {
    // AtomicUsize is lock-free on supported Linux targets. No allocation,
    // interpreter callback, errno-changing operation or cancellation occurs.
    let _ = STATE.fetch_update(Ordering::Relaxed, Ordering::Relaxed, toggled);
}

impl Owner {
    pub(super) fn reserve() -> io::Result<Self> {
        let generation = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value
                    .checked_add(1)
                    .filter(|value| *value <= usize::MAX >> 9)
            })
            .map_err(|_| io::Error::other("signal generation limit exceeded"))?;
        let token = generation << 9;
        STATE
            .compare_exchange(0, token, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "process signal guard already installed",
                )
            })?;
        Ok(Self {
            token,
            _thread: PhantomData,
        })
    }
    pub(super) fn handle(&self) -> Handle {
        Handle {
            token: self.token,
            _thread: PhantomData,
        }
    }
}

impl Handle {
    pub fn current(self) -> Option<u8> {
        let state = STATE.load(Ordering::Relaxed);
        (state & !LOW == self.token && state & ENABLED != 0).then_some((state & MASK) as u8)
    }
    pub fn publish(self, mask: u8) {
        let _ = STATE.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |state| {
            (state & !LOW == self.token && state & ENABLED != 0)
                .then_some((state & !MASK) | usize::from(mask))
        });
    }
    pub fn activate(self, mask: u8) -> io::Result<()> {
        let state = STATE.load(Ordering::Acquire);
        if state & !LOW != self.token {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "process signal guard was released",
            ));
        }
        if state & ENABLED != 0 {
            self.publish(mask);
            return Ok(());
        }
        STATE.store(self.token | ENABLED | usize::from(mask), Ordering::Release);
        let previous = unsafe { super::signal(10, caught as *const () as usize) };
        if previous == usize::MAX {
            STATE.store(self.token, Ordering::Release);
            return Err(io::Error::last_os_error());
        }
        PREVIOUS.store(previous, Ordering::Release);
        Ok(())
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        if STATE.load(Ordering::Acquire) & !LOW == self.token {
            let previous = PREVIOUS.swap(usize::MAX, Ordering::AcqRel);
            if previous != usize::MAX {
                unsafe { super::signal(10, previous) };
            }
            STATE.store(0, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn toggle_preserves_owner_and_never_enables_unpublished_state() {
        let token = 27 << 9;
        assert_eq!(toggled(0), None);
        assert_eq!(toggled(token), None);
        assert_eq!(toggled(token | ENABLED), Some(token | ENABLED | 1));
        for mask in 1..=255 {
            assert_eq!(toggled(token | ENABLED | mask), Some(token | ENABLED));
        }
    }
}
