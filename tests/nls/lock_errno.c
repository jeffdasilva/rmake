// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

// Test-only fault for blocking write locks on the owned fixture mutex.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

typedef int (*fcntl_fn)(int, int, ...);

static int borrowed_mutex(int fd)
{
    char descriptor[64], path[512];
    int length = snprintf(descriptor, sizeof descriptor, "/proc/self/fd/%d", fd);
    if (length < 0 || (size_t)length >= sizeof descriptor)
        return 0;
    ssize_t count = readlink(descriptor, path, sizeof path);
    const char suffix[] = "/mutex";
    return count >= (ssize_t)(sizeof suffix - 1)
        && !memcmp(path + count - (sizeof suffix - 1), suffix, sizeof suffix - 1);
}

static int pass(fcntl_fn next, int fd, int operation, va_list args)
{
    if (!next)
        _exit(91);
    switch (operation) {
    case F_GETFD:
    case F_GETFL:
    case F_GETOWN:
    case F_GETPIPE_SZ:
        return next(fd, operation);
    case F_GETLK:
    case F_SETLK:
    case F_SETLKW: {
        struct flock *lock = va_arg(args, struct flock *);
        if (operation == F_SETLKW && lock->l_type == F_WRLCK && borrowed_mutex(fd)) {
            errno = ENOLCK;
            return -1;
        }
        return next(fd, operation, lock);
    }
    case F_SETFD:
    case F_SETFL:
    case F_DUPFD:
    case F_DUPFD_CLOEXEC:
    case F_SETPIPE_SZ:
    case F_SETOWN:
        return next(fd, operation, va_arg(args, int));
    default:
        _exit(92);
    }
}

#define WRAP(name)                              \
    int name(int fd, int operation, ...)        \
    {                                           \
        fcntl_fn next = dlsym(RTLD_NEXT, #name);  \
        va_list args;                           \
        va_start(args, operation);              \
        int result = pass(next, fd, operation, args); \
        va_end(args);                           \
        return result;                          \
    }

WRAP(fcntl)
WRAP(fcntl64)
