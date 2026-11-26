/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Observe named stdin storage without substituting filesystem results. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

static int witness(char *name, int result) {
    int saved = errno;
    const char *path = getenv("PROBE_CREATE_RECORD");
    if (path) {
        int fd = open(path, O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC, 0600);
        if (fd < 0 || dprintf(fd, "%s\t%d\n", name, result < 0 ? saved : 0) < 0 || close(fd) < 0)
            abort();
    }
    errno = saved;
    return result;
}

int mkstemp(char *name) {
    int (*next)(char *) = dlsym(RTLD_NEXT, "mkstemp");
    if (!next)
        abort();
    return witness(name, next(name));
}

int mkostemp(char *name, int flags) {
    int (*next)(char *, int) = dlsym(RTLD_NEXT, "mkostemp");
    if (!next)
        abort();
    return witness(name, next(name, flags));
}
