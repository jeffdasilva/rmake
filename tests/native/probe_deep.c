/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original native expansion depth probe; no GNU header is included. */
#include <stdio.h>
#include <stdlib.h>
struct Place { const char *file; unsigned long line; };
typedef char *(*Callback)(const char *, unsigned int, char **);
extern char *gmk_expand(const char *);
extern void gmk_add_function(const char *, Callback, unsigned int, unsigned int, unsigned int);
int plugin_is_GPL_compatible = 1;
static char *deep(const char *name, unsigned int argc, char **argv) {
    (void)name;
    unsigned long number = argc ? strtoul(argv[0], NULL, 10) : 0;
    if (!number) return gmk_expand("$(file >>seen,leaf)$(VALUE)");
    char expression[96];
    snprintf(expression, sizeof expression, "$(deep %lu)", number - 1);
    return gmk_expand(expression);
}
int probe_deep_gmk_setup(const struct Place *ignored) {
    (void)ignored;
    gmk_add_function("deep", deep, 1, 1, 0);
    return 1;
}
