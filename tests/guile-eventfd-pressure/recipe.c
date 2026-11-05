/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original exec witness and bounded recipe barrier. */
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <time.h>
#include <unistd.h>

static int ready(void) {
    static const char bytes[] = "recipe-executed\n";
    int fd = open("ready.tmp", O_WRONLY | O_CREAT | O_EXCL, 0644);
    if (fd < 0)
        return 0;
    ssize_t result = write(fd, bytes, sizeof bytes - 1);
    int closed = close(fd);
    return result == sizeof bytes - 1 && closed == 0 && rename("ready.tmp", "ready") == 0;
}

int main(int argc, char **argv) {
    struct rlimit actual;
    if (argc != 3 || getrlimit(RLIMIT_NOFILE, &actual) != 0 ||
        actual.rlim_cur != (rlim_t)atoi(argv[1]) || actual.rlim_max != actual.rlim_cur ||
        !ready())
        return 71;
    if (strcmp(argv[2], "plain") == 0) {
        puts("recipe completed");
        return 0;
    }
    const struct timespec tick = {0, 1000000};
    for (unsigned int attempt = 0; attempt < 1800; ++attempt) {
        int fd = open("done", O_RDONLY | O_CLOEXEC);
        if (fd >= 0) {
            char bytes[16];
            ssize_t count = read(fd, bytes, sizeof bytes);
            int closed = close(fd);
            if (closed != 0 || count != 7 || memcmp(bytes, "worker\n", 7) != 0)
                return 72;
            puts("recipe saw worker");
            return 0;
        }
        if (errno != ENOENT)
            return 73;
        nanosleep(&tick, NULL);
    }
    return 74;
}
