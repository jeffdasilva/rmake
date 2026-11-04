/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original stdio/evaluator ordering fixture, without GNU headers. */
#include <stdio.h>
#include <string.h>
struct Source { const char *filename; unsigned long line; };
typedef char *(*Function)(const char *, unsigned int, char **);
extern char *gmk_alloc(unsigned int);
extern void gmk_free(char *);
extern char *gmk_expand(const char *);
extern void gmk_add_function(const char *, Function, unsigned int, unsigned int, unsigned int);
int plugin_is_GPL_compatible = 1;
static void expand_and_release(const char *text) { char *result = gmk_expand(text); gmk_free(result); }
static char *emit(const char *name, unsigned int argc, char **argv) {
    (void)name; (void)argc; (void)argv;
    printf("callback-partial");
    expand_and_release("$(warning callback-warning)");
    fwrite("-tail\n", 1, 6, stdout);
    expand_and_release("$(info callback-info)");
    char *result = gmk_alloc(6); memcpy(result, "stdio", 6); return result;
}
int probe_stdio_gmk_setup(const struct Source *location) {
    (void)location;
    printf("setup-partial");
    expand_and_release("$(warning setup-warning)");
    fwrite("-tail\n", 1, 6, stdout);
    expand_and_release("$(info setup-info)");
    gmk_add_function("probe_stdio", emit, 0, 1, 0);
    return 1;
}
