/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original witness: every recipe must enter before any can finish. */
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>
#include <time.h>
#include <unistd.h>

static int record(const char *stage, int index, rlim_t limit) {
    char path[32], data[64];
    snprintf(path, sizeof path, "%s-%d", stage, index);
    int size = snprintf(data, sizeof data, "%s %d nofile=%llu\n",
                        stage, index, (unsigned long long)limit);
    int fd = open(path, O_WRONLY | O_CREAT | O_EXCL, 0644);
    if (fd < 0)
        return 1;
    ssize_t written = write(fd, data, (size_t)size);
    int closed = close(fd);
    return written != size || closed != 0;
}

static int all_ready(int count) {
    for (int index = 0; index < count; ++index) {
        char path[32];
        snprintf(path, sizeof path, "ready-%d", index);
        if (access(path, F_OK) != 0)
            return 0;
    }
    return 1;
}

int main(int argc, char **argv) {
    if (argc != 4)
        return 31;
    int index = atoi(argv[1]), count = atoi(argv[2]);
    unsigned long limit = strtoul(argv[3], NULL, 10);
    struct rlimit actual;
    if (index < 0 || index >= count || count < 1 || count > 32 ||
        getrlimit(RLIMIT_NOFILE, &actual) != 0 ||
        actual.rlim_cur != limit || actual.rlim_max != limit)
        return 32;
    if (record("ready", index, actual.rlim_cur) != 0)
        return 33;
    const struct timespec delay = {0, 1000000};
    for (int attempt = 0; attempt < 1000; ++attempt) {
        if (all_ready(count))
            return record("done", index, actual.rlim_cur) == 0 ? 0 : 34;
        if (nanosleep(&delay, NULL) != 0 && errno != EINTR)
            return 35;
    }
    return 36;
}
