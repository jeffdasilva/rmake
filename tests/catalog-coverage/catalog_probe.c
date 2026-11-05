/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original public gettext selection witness. */
#include <libintl.h>
#include <stddef.h>
#include <string.h>

typedef char *(*callback)(const char *, unsigned int, char **);
extern void gmk_add_function(const char *, callback, unsigned int, unsigned int, unsigned int);
extern char *gmk_alloc(unsigned int);
int plugin_is_GPL_compatible;

static char *selected(const char *name, unsigned int count, char **arguments) {
    (void)name;
    (void)count;
    const char *translated = gettext(arguments[0]);
    size_t length = strlen(translated) + 1;
    char *copy = gmk_alloc((unsigned int)length);
    memcpy(copy, translated, length);
    return copy;
}

int catalog_probe_gmk_setup(const void *location) {
    (void)location;
    gmk_add_function("catalog_probe", selected, 1, 1, 0);
    return 1;
}
