/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva */
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

int plugin_is_GPL_compatible = 1;

static void rebind_stdout(void) {
    int fd = open("seen", O_WRONLY | O_APPEND);
    if (fd < 0)
        _exit(70);
    if (fd != STDOUT_FILENO && (dup2(fd, STDOUT_FILENO) < 0 || close(fd) < 0))
        _exit(71);
    if (write(STDOUT_FILENO, "rebound\n", 8) != 8)
        _exit(72);
}

int stdout_rebind_gmk_setup(void) {
    return atexit(rebind_stdout) == 0;
}

__attribute__((destructor)) static void inspect_stdout(void) {
    int state = fcntl(STDOUT_FILENO, F_GETFD);
    const char *result = state >= 0 ? "open" : errno == EBADF ? "closed" : "error";
    if (dprintf(STDERR_FILENO, "finalizer fd1=%s\n", result) < 0)
        _exit(73);
}
