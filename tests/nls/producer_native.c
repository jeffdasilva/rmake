/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original fixture for user text emitted from a native callback. */
#include <stdio.h>
#include <stdlib.h>

typedef char *(*make_function)(const char *, unsigned int, char **);
extern void gmk_add_function(const char *, make_function, unsigned int, unsigned int, unsigned int);
extern void gmk_eval(const char *, const void *);

int plugin_is_GPL_compatible;

static void mark(const char *message)
{
    FILE *seen = fopen("seen", "a");
    if (!seen)
        abort();
    if (fputs(message, seen) == EOF || fclose(seen))
        abort();
}

static char *raw_error(const char *name, unsigned int argc, char **argv)
{
    (void)name;
    (void)argc;
    (void)argv;
    mark("native-before\n");
    gmk_eval("$(error file: missing filename)", NULL);
    mark("native-forbidden\n");
    return NULL;
}

int producer_native_gmk_setup(const void *location)
{
    (void)location;
    gmk_add_function("nls_native_raw_error", raw_error, 0, 1, 0);
    return 1;
}
