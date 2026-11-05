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
extern char *gmk_alloc(unsigned int);
int plugin_is_GPL_compatible;

static char out_buffer[256], err_buffer[256], private_buffer[256];
static int saved_stderr = -1;

static void witness(const char *mode) {
    char line[80];
    int size = snprintf(line, sizeof line, "armed-%s\n", mode);
    int fd = open("seen", O_WRONLY | O_APPEND | O_CLOEXEC);
    if (size <= 0 || (size_t)size >= sizeof line || fd < 0
        || write(fd, line, (size_t)size) != size || close(fd) < 0)
        _exit(90);
}

static char *stage_spawn(const char *name, unsigned int count, char **args) {
    (void)name;
    if (count != 1)
        _exit(91);
    const char *mode = args[0];
    if (!strcmp(mode, "private") || !strcmp(mode, "private-full")) {
        FILE *stream = fopen(!strcmp(mode, "private") ? "private-stream" : "/dev/full", "we");
        if (!stream || setvbuf(stream, private_buffer, _IOFBF, sizeof private_buffer)
            || fputs("private bytes", stream) == EOF)
            _exit(92);
        /* libc owns this stream until normal process exit. */
    } else if (!strcmp(mode, "public")) {
        if (setvbuf(stdout, out_buffer, _IOFBF, sizeof out_buffer)
            || setvbuf(stderr, err_buffer, _IOFBF, sizeof err_buffer)
            || fputs("old-out:", stdout) == EOF || fputs("old-err:", stderr) == EOF)
            _exit(93);
    } else if (!strcmp(mode, "stderr-full")) {
        saved_stderr = fcntl(STDERR_FILENO, F_DUPFD_CLOEXEC, 20);
        int full = open("/dev/full", O_WRONLY | O_CLOEXEC);
        if (saved_stderr < 0 || full < 0 || dup2(full, STDERR_FILENO) < 0 || close(full) < 0
            || setvbuf(stderr, err_buffer, _IOFBF, sizeof err_buffer)
            || fputs("buffered error output", stderr) == EOF)
            _exit(94);
    } else {
        _exit(95);
    }
    witness(mode);
    return NULL;
}

static char *restore_spawn(const char *name, unsigned int count, char **args) {
    (void)name; (void)count; (void)args;
    const char *state = ferror(stderr) ? "error" : "clean";
    /* Retire pending fixture bytes against the same failing destination. */
    (void)fflush(stderr);
    clearerr(stderr);
    if (saved_stderr < 0 || dup2(saved_stderr, STDERR_FILENO) < 0 || close(saved_stderr) < 0)
        _exit(96);
    saved_stderr = -1;
    size_t length = strlen(state) + 1;
    char *result = gmk_alloc((unsigned int)length);
    memcpy(result, state, length);
    return result;
}

int spawn_probe_gmk_setup(const void *location) {
    (void)location;
    gmk_add_function("stage_spawn", stage_spawn, 1, 1, 0);
    gmk_add_function("restore_spawn", restore_spawn, 0, 1, 0);
    return 1;
}
