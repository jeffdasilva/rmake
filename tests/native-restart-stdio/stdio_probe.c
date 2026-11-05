/* SPDX-License-Identifier: GPL-3.0-or-later */
/* Copyright 2026 Jeff DaSilva */

#define _GNU_SOURCE
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

typedef char *(*callback)(const char *, unsigned int, char **);
extern void gmk_add_function(const char *, callback, unsigned int, unsigned int, unsigned int);
int plugin_is_GPL_compatible;

static const char saved_name[] = "RMAKE_RESTART_STDOUT_TEST";
static char out_buffer[256], err_buffer[256], private_buffer[256];

static void witness(const char *mode) {
    char line[80];
    int size = snprintf(line, sizeof line, "armed-%s\n", mode);
    int fd = open("seen", O_WRONLY | O_APPEND | O_CLOEXEC);
    if (size <= 0 || (size_t)size >= sizeof line || fd < 0 || write(fd, line, (size_t)size) != size
        || close(fd) < 0)
        _exit(90);
}

static char *stage_stdio(const char *name, unsigned int count, char **args) {
    (void)name;
    if (count != 1)
        _exit(91);
    const char *mode = args[0];
    if (!strcmp(mode, "private") || !strcmp(mode, "private-full")) {
        FILE *stream = fopen(!strcmp(mode, "private") ? "private-stream" : "/dev/full", "we");
        if (!stream || setvbuf(stream, private_buffer, _IOFBF, sizeof private_buffer)
            || fputs("private buffered bytes", stream) == EOF)
            _exit(92);
        /* Exec retires the fixture-owned stream. */
    } else if (!strcmp(mode, "public")) {
        if (setvbuf(stdout, out_buffer, _IOFBF, sizeof out_buffer)
            || setvbuf(stderr, err_buffer, _IOFBF, sizeof err_buffer)
            || fputs("old-out:", stdout) == EOF || fputs("old-err:", stderr) == EOF)
            _exit(93);
    } else if (!strcmp(mode, "full")) {
        int saved = fcntl(STDOUT_FILENO, F_DUPFD, 20);
        int full = open("/dev/full", O_WRONLY | O_CLOEXEC);
        char number[32];
        if (saved < 0 || full < 0 || snprintf(number, sizeof number, "%d", saved) <= 0
            || setenv(saved_name, number, 1) || dup2(full, STDOUT_FILENO) < 0 || close(full) < 0
            || setvbuf(stdout, out_buffer, _IOFBF, sizeof out_buffer)
            || fputs("discarded error output", stdout) == EOF)
            _exit(94);
    } else {
        _exit(95);
    }
    witness(mode);
    return NULL;
}

int stdio_probe_gmk_setup(const void *location) {
    (void)location;
    const char *saved = getenv(saved_name);
    if (saved) {
        char *end;
        long fd = strtol(saved, &end, 10);
        if (*end || fd < 20 || fd > 1024 || dup2((int)fd, STDOUT_FILENO) < 0 || close((int)fd) < 0
            || unsetenv(saved_name))
            _exit(96);
    }
    gmk_add_function("stage_stdio", stage_stdio, 1, 1, 0);
    return 1;
}
