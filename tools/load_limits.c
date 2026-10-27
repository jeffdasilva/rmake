// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva
// Test-only Linux/glibc load observations. Original helper, no Make semantics.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
static int load_fd = -1, interrupted, noted_proc, noted_average;
static void *next(const char *name) { void *p = dlsym(RTLD_NEXT, name); if (!p) _exit(123); return p; }
static int mode(const char *value) { const char *m = getenv("RMAKE_TEST_LOAD"); return m && !strcmp(m, value); }
static void note(const char *text) {
    int saved = errno;
    int (*real_open)(const char *, int, ...) = next("open");
    const char *path = getenv("RMAKE_LOAD_TRACE");
    int fd = path ? real_open(path, O_WRONLY|O_CREAT|O_APPEND|O_CLOEXEC, 0600) : -1;
    ssize_t (*real_write)(int, const void *, size_t) = next("write");
    if (fd < 0 || real_write(fd, text, strlen(text)) != (ssize_t)strlen(text)) _exit(124);
    close(fd); errno = saved;
}
static int observed_open(const char *symbol, const char *path, int flags, mode_t permissions) {
    int (*real_open)(const char *, int, ...) = next(symbol);
    if (path && !strcmp(path, "/proc/loadavg") && getenv("RMAKE_TEST_LOAD")) {
        if (!noted_proc++) note("proc\n");
        if (mode("open-denied")) { errno = EACCES; return -1; }
        load_fd = real_open("load.input", flags, permissions);
        return load_fd;
    }
    return real_open(path, flags, permissions);
}
int open(const char *path, int flags, ...) {
    mode_t permissions = 0;
    if ((flags & O_CREAT) || (flags & O_TMPFILE) == O_TMPFILE) { va_list a; va_start(a, flags); permissions = va_arg(a, int); va_end(a); }
    return observed_open("open", path, flags, permissions);
}
int open64(const char *path, int flags, ...) {
    mode_t permissions = 0;
    if ((flags & O_CREAT) || (flags & O_TMPFILE) == O_TMPFILE) { va_list a; va_start(a, flags); permissions = va_arg(a, int); va_end(a); }
    return observed_open("open64", path, flags, permissions);
}
ssize_t read(int fd, void *bytes, size_t length) {
    ssize_t (*real_read)(int, void *, size_t) = next("read");
    if (fd == load_fd && !interrupted && (mode("read-eintr") || mode("read-eio"))) {
        interrupted = 1; note("read-fault\n"); errno = mode("read-eintr") ? EINTR : EIO; return -1;
    }
    return real_read(fd, bytes, length);
}
int getloadavg(double *values, int count) {
    if (!getenv("RMAKE_TEST_LOAD")) { int (*real)(double *, int) = next("getloadavg"); return real(values, count); }
    if (!noted_average++) note("average\n");
    const char *value = getenv("RMAKE_LOAD_AVERAGE");
    if (value && !strcmp(value, "eio")) { errno = EIO; return -1; }
    if (value && !strcmp(value, "unsupported")) { errno = 0; return -1; }
    if (count < 1) return 0;
    values[0] = value ? strtod(value, NULL) : 100.0;
    return 1;
}
