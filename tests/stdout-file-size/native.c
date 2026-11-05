/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original public stdio and process-finalization witness. */
#define _POSIX_C_SOURCE 200809L
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

int plugin_is_GPL_compatible;

static void mark(const char *path, const char *bytes)
{
    int fd = open(path, O_WRONLY | O_CREAT | O_EXCL, 0644);
    if (fd >= 0) {
        if (write(fd, bytes, 2) != 2)
            (void)unlink(path);
        (void)close(fd);
    }
}

static void before_close(void)
{
    mark("atexit-seen", fcntl(STDOUT_FILENO, F_GETFD) >= 0
        ? (ferror(stdout) ? "E\n" : "O\n") : "C\n");
}

__attribute__((destructor))
static void after_close(void)
{
    mark("fini-seen", fcntl(STDOUT_FILENO, F_GETFD) < 0 ? "C\n" : "O\n");
}

int native_probe_gmk_setup(void *location)
{
    (void)location;
    if (atexit(before_close))
        return 0;
    if (getenv("NATIVE_WRITE")) {
        (void)fputs("ABCDEFGH\n", stdout);
        (void)fflush(stdout);
    }
    return 1;
}
