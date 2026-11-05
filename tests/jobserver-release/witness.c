/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original phase and controlled child-completion witness. */
#define _POSIX_C_SOURCE 200809L
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

static int mark(const char *path)
{
    int fd = open(path, O_WRONLY | O_CREAT | O_EXCL, 0644);
    return fd >= 0 && close(fd) == 0;
}

static int await(const char *path)
{
    const struct timespec delay = {0, 1000000};
    for (int count = 0; count < 3400; ++count) {
        if (access(path, F_OK) == 0)
            return 1;
        nanosleep(&delay, NULL);
    }
    return 0;
}

int main(int argc, char **argv)
{
    if (argc != 2)
        return 91;
    const char *mode = argv[1];
    if (!strcmp(mode, "a")) {
        if (dprintf(1, "a:start\n") < 0 || !mark("a.ready")
            || !await("a.release") || dprintf(1, "a:done\n") < 0)
            return 92;
        return 0;
    }
    if (!await("a.ready"))
        return 93;
    if (!strcmp(mode, "ignored")) {
        if (dprintf(1, "b:ignored-command\n") < 0)
            return 94;
        return 7;
    }
    if (dprintf(1, "b:%s\n", mode) < 0 || !mark("b.held") || !await("b.release"))
        return 95;
    if (!strcmp(mode, "signal")) {
        if (raise(SIGTERM) != 0)
            return 96;
        return 97;
    }
    if (!strcmp(mode, "first") && !mark("b.finished"))
        return 99;
    return dprintf(1, "b:done\n") < 0 ? 98 : 0;
}
