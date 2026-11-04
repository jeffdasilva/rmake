/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original public-ABI line-location probe; no GNU header is included. */
#include <stdlib.h>
struct Place { const char *file; unsigned long line; };
extern void gmk_eval(const char *, const struct Place *);
int plugin_is_GPL_compatible = 1;
int probe_line_gmk_setup(const struct Place *ignored) {
    (void)ignored;
    const char *number = getenv("PROBE_LINE");
    struct Place location = {"native-lines.mk", number ? strtoul(number, NULL, 10) : 0};
    gmk_eval("$(warning first)\n$(warning second)\n", &location);
    return 1;
}
