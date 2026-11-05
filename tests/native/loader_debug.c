/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original bounded witness for native loader diagnostics. */
#define _POSIX_C_SOURCE 200809L
#include <fcntl.h>
#include <stdio.h>
#include <unistd.h>

int plugin_is_GPL_compatible;

static void journal(const char *bytes, size_t length) {
    int fd = open("seen", O_WRONLY | O_APPEND | O_CLOEXEC);
    if (fd < 0 || write(fd, bytes, length) != (ssize_t)length || close(fd) < 0)
        _exit(90);
}

__attribute__((constructor)) static void started(void) {
    journal("ctor\n", sizeof "ctor\n" - 1);
    if (puts("constructor stdout") < 0)
        _exit(91);
}

__attribute__((destructor)) static void finished(void) {
    journal("dtor\n", sizeof "dtor\n" - 1);
}

int probe_gmk_setup(const void *location) {
    (void)location;
    journal("setup\n", sizeof "setup\n" - 1);
    if (puts("setup stdout") < 0)
        _exit(92);
    return -1;
}
