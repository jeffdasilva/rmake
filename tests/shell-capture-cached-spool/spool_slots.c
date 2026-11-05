/* SPDX-License-Identifier: GPL-3.0-or-later */
/* Copyright 2026 Jeff DaSilva */

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stddef.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

typedef char *(*callback)(const char *, unsigned int, char **);
extern void gmk_add_function(const char *, callback, unsigned int, unsigned int, unsigned int);
extern char *gmk_alloc(unsigned int);
int plugin_is_GPL_compatible;

static int saved[3] = {-1, -1, -1};
static int flags[3];
static int close_all;

static char *spool_close(const char *function, unsigned int argc, char **argv) {
    (void)function;
    if (argc != 1)
        _exit(90);
    close_all = !strcmp(argv[0], "all");
    for (int fd = 0; fd != 3; ++fd) {
        if (fd == 1 && !close_all)
            continue;
        flags[fd] = fcntl(fd, F_GETFD);
        saved[fd] = fcntl(fd, F_DUPFD_CLOEXEC, 20);
        if (flags[fd] < 0 || saved[fd] < 0)
            _exit(91);
    }
    for (int fd = 0; fd != 3; ++fd)
        if (saved[fd] >= 0 && close(fd) < 0)
            _exit(92);
    return NULL;
}

static int absent(int fd) {
    return fcntl(fd, F_GETFD) < 0 && errno == EBADF;
}

static int one_pipe(int writer) {
    struct stat a, b;
    int af = fcntl(0, F_GETFD), bf = fcntl(writer, F_GETFD);
    return af >= 0 && bf >= 0 && (af & FD_CLOEXEC) && (bf & FD_CLOEXEC)
        && fstat(0, &a) == 0 && fstat(writer, &b) == 0
        && S_ISFIFO(a.st_mode) && S_ISFIFO(b.st_mode)
        && a.st_dev == b.st_dev && a.st_ino == b.st_ino
        && (fcntl(0, F_GETFL) & O_ACCMODE) == O_RDONLY
        && (fcntl(writer, F_GETFL) & O_ACCMODE) == O_WRONLY;
}

static char *spool_restore(const char *function, unsigned int argc, char **argv) {
    (void)function;
    (void)argc;
    (void)argv;
    const char *state = "other";
    if (absent(0) && absent(2) && (close_all ? absent(1) : !absent(1)))
        state = "closed";
    if (one_pipe(close_all ? 1 : 2) && (close_all ? absent(2) : !absent(1)))
        state = close_all ? "pipe-01" : "pipe-02";
    for (int fd = 0; fd != 3; ++fd) {
        if (saved[fd] < 0)
            continue;
        if (dup2(saved[fd], fd) < 0 || fcntl(fd, F_SETFD, flags[fd]) < 0
            || close(saved[fd]) < 0)
            _exit(93);
        saved[fd] = -1;
    }
    size_t length = strlen(state) + 1;
    char *value = gmk_alloc((unsigned int)length);
    memcpy(value, state, length);
    return value;
}

int spool_slots_gmk_setup(const void *location) {
    (void)location;
    gmk_add_function("spool_close", spool_close, 1, 1, 0);
    gmk_add_function("spool_restore", spool_restore, 0, 1, 0);
    return 1;
}
