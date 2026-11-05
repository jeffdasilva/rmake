/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original public native API fixture. */
#include <stddef.h>

typedef char *(*callback)(const char *, unsigned int, char **);
extern char *gmk_expand(const char *);
extern void gmk_eval(const char *, const void *);
extern void gmk_add_function(const char *, callback, unsigned int, unsigned int, unsigned int);
int plugin_is_GPL_compatible;

static char *ordinary(const char *name, unsigned int argc, char **argv) {
    (void)name; (void)argc; (void)argv;
    gmk_eval("X := native", NULL);
    return gmk_expand("$(X)");
}

int callback_gmk_setup(const void *location) {
    (void)location;
    gmk_add_function("ordinary", ordinary, 0, 1, 0);
    return 1;
}
