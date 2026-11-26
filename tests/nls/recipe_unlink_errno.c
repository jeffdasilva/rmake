// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

// Test-only unlink denial for the two owned recipe-target names.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <string.h>
#include <unistd.h>

int unlink(const char *path)
{
    if (!strcmp(path, "guard/target") || !strcmp(path, "guard/raw_\xff")) {
        errno = EACCES;
        return -1;
    }
    int (*next)(const char *) = dlsym(RTLD_NEXT, "unlink");
    if (!next)
        _exit(91);
    return next(path);
}
