/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva */
#include <stdlib.h>

struct Source { const char *filename; unsigned long line; };
extern void gmk_eval(const char *, const struct Source *);
int plugin_is_GPL_compatible = 1;

__attribute__((destructor)) static void finalize(void) {
    gmk_eval("$(warning finalizer-location)\n"
             "$(file >>seen,scope=[$@|$<|$(origin .LOADED)|$(.LOADED)|$(0)|$(1)])",
             NULL);
}

int fatal_scope_gmk_setup(const struct Source *location) {
    (void)location;
    if (getenv("PROBE_FATAL_SCOPE_SETUP")) {
        gmk_eval("$(file >>seen,setup-enter)\n$(error setup-stop)", NULL);
        gmk_eval("$(file >>seen,setup-return)", NULL);
    }
    return 1;
}
