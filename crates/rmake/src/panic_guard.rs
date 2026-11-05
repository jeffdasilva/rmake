// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

pub(crate) fn install() {
    // This hook runs before unwinding a paused foreign call or its real host.
    std::panic::set_hook(Box::new(|_| rmake_linux::guile::abort_host()));
}
