/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original registration boundary fixture. No GNU header is included. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
struct Source { const char *filename; unsigned long line; };
typedef char *(*Function)(const char *, unsigned int, char **);
extern char *gmk_alloc(unsigned int);
extern void gmk_add_function(const char *, Function, unsigned int, unsigned int, unsigned int);
int plugin_is_GPL_compatible = 1;
static void mark(const char *text) {
    FILE *stream = fopen("seen", "a");
    if (!stream) exit(90);
    fputs(text, stream); fputc('\n', stream);
    if (fclose(stream)) exit(91);
}
static char *callback(const char *name, unsigned int argc, char **argv) {
    (void)name; (void)argc; (void)argv;
    mark("called");
    char *result = gmk_alloc(9); memcpy(result, "accepted", 9); return result;
}
int registration_gmk_setup(const struct Source *location) {
    (void)location;
    const char *name = getenv("PROBE_REG_NAME");
    const char *minimum = getenv("PROBE_REG_MIN");
    const char *maximum = getenv("PROBE_REG_MAX");
    const char *flags = getenv("PROBE_REG_FLAGS");
    mark("register-before");
    gmk_add_function(name ? name : "native_registered", callback,
                     minimum ? (unsigned int)strtoul(minimum, NULL, 10) : 0,
                     maximum ? (unsigned int)strtoul(maximum, NULL, 10) : 1,
                     flags ? (unsigned int)strtoul(flags, NULL, 10) : 0);
    mark("register-after");
    return 1;
}
