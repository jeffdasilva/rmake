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

static int first_slot;
static int saved[2] = {-1, -1};
static int original_flags[2];

static char *close_slots(const char *name, unsigned int count, char **args) {
    (void)name;
    first_slot = count && !strcmp(args[0], "stdout-only") ? 1 : 0;
    for (int fd = first_slot; fd < 2; ++fd) {
        original_flags[fd] = fcntl(fd, F_GETFD);
        saved[fd] = fcntl(fd, F_DUPFD_CLOEXEC, 20);
        if (original_flags[fd] < 0 || saved[fd] < 0)
            _exit(90);
    }
    for (int fd = first_slot; fd < 2; ++fd)
        if (close(fd) < 0)
            _exit(91);
    return NULL;
}

static char *restore_slots(const char *name, unsigned int count, char **args) {
    (void)name;
    (void)count;
    (void)args;
    struct stat reader, writer;
    int a = fcntl(0, F_GETFD);
    int a_closed = a < 0 && errno == EBADF;
    int b = fcntl(1, F_GETFD);
    int b_closed = b < 0 && errno == EBADF;
    const char *state = "other";
    if ((first_slot ? a >= 0 : a_closed) && b_closed)
        state = "closed";
    if (a >= 0 && b >= 0 && (a & FD_CLOEXEC) && (b & FD_CLOEXEC)
        && fstat(0, &reader) == 0 && fstat(1, &writer) == 0
        && S_ISFIFO(reader.st_mode) && S_ISFIFO(writer.st_mode)
        && reader.st_dev == writer.st_dev && reader.st_ino == writer.st_ino
        && (fcntl(0, F_GETFL) & O_ACCMODE) == O_RDONLY
        && (fcntl(1, F_GETFL) & O_ACCMODE) == O_WRONLY)
        state = "one-cloexec-pipe";
    for (int fd = first_slot; fd < 2; ++fd) {
        if (dup2(saved[fd], fd) < 0 || fcntl(fd, F_SETFD, original_flags[fd]) < 0
            || close(saved[fd]) < 0)
            _exit(92);
        saved[fd] = -1;
    }
    size_t size = strlen(state) + 1;
    char *result = gmk_alloc((unsigned int)size);
    memcpy(result, state, size);
    return result;
}

int low_slots_gmk_setup(const void *location) {
    (void)location;
    gmk_add_function("close_slots", close_slots, 1, 1, 0);
    gmk_add_function("restore_slots", restore_slots, 0, 1, 0);
    return 1;
}
