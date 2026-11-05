/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original native setup return and unload witness. */
#define _GNU_SOURCE
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

typedef char *(*callback)(const char *, unsigned int, char **);
extern void gmk_add_function(const char *, callback, unsigned int, unsigned int, unsigned int);
extern char *gmk_alloc(unsigned int);
int plugin_is_GPL_compatible;

static void witness(const char *event) {
    char line[64];
    int size = snprintf(line, sizeof line, "%s-%s\n", event, IMAGE);
    int fd = open("seen", O_WRONLY | O_APPEND | O_CLOEXEC);
    if (size <= 0 || (size_t)size >= sizeof line || fd < 0
        || write(fd, line, (size_t)size) != size || close(fd) < 0)
        _exit(90);
}

__attribute__((constructor)) static void initialized(void) { witness("ctor"); }
__attribute__((destructor)) static void finished(void) { witness("dtor"); }

static char *image(const char *name, unsigned int count, char **argv) {
    (void)name; (void)count; (void)argv;
    char *result = gmk_alloc(sizeof IMAGE);
    memcpy(result, IMAGE, sizeof IMAGE);
    return result;
}

static int setup(void) {
    witness("setup");
    gmk_add_function("probe_image", image, 0, 1, 0);
    return SETUP_RESULT;
}

int probe_gmk_setup(const void *location) { (void)location; return setup(); }
int alternate_setup(const void *location) { (void)location; return setup(); }
