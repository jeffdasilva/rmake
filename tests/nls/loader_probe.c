/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original configurable loader and registration witness. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef char *(*make_function)(const char *, unsigned int, char **);
extern void gmk_add_function(const char *, make_function, unsigned int, unsigned int, unsigned int);
#ifndef OMIT_LICENSE
int plugin_is_GPL_compatible;
#endif
#ifndef PROBE_KIND
#define PROBE_KIND 0
#endif

static void mark(const char *message)
{
    FILE *seen = fopen("seen", "a");
    if (!seen)
        abort();
    if (fputs(message, seen) == EOF || fclose(seen))
        abort();
}

#if PROBE_KIND > 0
static char *native_noop(const char *name, unsigned int argc, char **argv)
{
    (void)name;
    (void)argc;
    (void)argv;
    return NULL;
}
#endif

int loader_probe_gmk_setup(const void *location)
{
    (void)location;
    mark("setup\n");
#if PROBE_KIND == 0
    return 0;
#elif PROBE_KIND == 1
    gmk_add_function("", native_noop, 0, 1, 0);
#elif PROBE_KIND == 2
    gmk_add_function("bad\xff", native_noop, 0, 1, 0);
#elif PROBE_KIND == 3
    char name[257];
    memset(name, 'x', sizeof(name) - 1);
    name[sizeof(name) - 1] = '\0';
    gmk_add_function(name, native_noop, 0, 1, 0);
#elif PROBE_KIND == 4
    gmk_add_function("native_min", native_noop, 256, 0, 0);
#elif PROBE_KIND == 5
    gmk_add_function("native_max", native_noop, 2, 1, 0);
#else
#error Unknown fixture mode
#endif
#if PROBE_KIND > 0
    mark("forbidden\n");
    return 1;
#endif
}
