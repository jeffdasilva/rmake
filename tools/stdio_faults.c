// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva.
// Original test-only Linux/glibc interposer. No GNU Make implementation copied.
// Only the literal fixture basename effect-probe is faulted. The journal proves
// that a requested fault happened; it preserves errno and uses no stdio calls.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

static int interrupted;
static FILE *fault_stream;
static int close_calls;

static int selected(const char *path) {
    // Rust probes statx availability with a null pathname before normal I/O.
    if (!path)
        return 0;
    const char *name = strrchr(path, '/');
    return strcmp(name ? name + 1 : path, "effect-probe") == 0;
}

static int mode_is(const char *name) {
    const char *mode = getenv("RMAKE_STDIO_FAULT");
    return mode && strcmp(mode, name) == 0;
}

static void note(const char *text) {
    int saved = errno;
    int fd = open("fault.log", O_WRONLY | O_CREAT | O_APPEND, 0644);
    if (fd < 0 || write(fd, text, strlen(text)) != (ssize_t)strlen(text))
        _exit(120);
    if (close(fd) != 0)
        _exit(121);
    errno = saved;
}

static void *symbol(const char *name) {
    void *address = dlsym(RTLD_NEXT, name);
    if (!address)
        _exit(122);
    return address;
}

static int interrupt_once(const char *path, const char *mode) {
    if (selected(path) && mode_is(mode) && !interrupted) {
        interrupted = 1;
        note("interrupted\n");
        errno = EINTR;
        return 1;
    }
    return 0;
}

struct read_cookie {
    FILE *backing;
    unsigned calls;
};

static ssize_t cookie_read(void *opaque, char *buffer, size_t size) {
    struct read_cookie *cookie = opaque;
    unsigned call = cookie->calls++;
    if (call == 1) {
        note("partial-read-fault\n");
        errno = mode_is("read-eintr") ? EINTR : EIO;
        return -1;
    }
    // First return a real prefix, then fail, then let libc read the remainder.
    if (call == 0 && size > 3)
        size = 3;
    return read(fileno(cookie->backing), buffer, size);
}

static int cookie_close(void *opaque) {
    struct read_cookie *cookie = opaque;
    int (*next)(FILE *) = symbol("fclose");
    int result = next(cookie->backing);
    free(cookie);
    return result;
}

FILE *fopen(const char *path, const char *mode) {
    FILE *(*next)(const char *, const char *) = symbol("fopen");
    if (interrupt_once(path, "open-eintr"))
        return NULL;
    FILE *stream = next(path, mode);
    if (!stream || !selected(path))
        return stream;
    if (mode[0] == 'r' && (mode_is("read-eintr") || mode_is("read-eio"))) {
        struct read_cookie *cookie = calloc(1, sizeof(*cookie));
        if (!cookie)
            _exit(123);
        cookie->backing = stream;
        cookie_io_functions_t operations = { .read = cookie_read, .close = cookie_close };
        stream = fopencookie(cookie, "r", operations);
        if (!stream)
            _exit(124);
    }
    fault_stream = stream;
    return stream;
}

int fputs(const char *text, FILE *stream) {
    int (*next)(const char *, FILE *) = symbol("fputs");
    int result = next(text, stream);
    if (stream == fault_stream && mode_is("write-eio")) {
        // Accepted bytes remain buffered. Fatal cleanup must preserve them.
        note("write-fault\n");
        errno = EIO;
        return EOF;
    }
    return result;
}

int fclose(FILE *stream) {
    int (*next)(FILE *) = symbol("fclose");
    if (stream == fault_stream && mode_is("close-eio")) {
        if (++close_calls != 1) {
            // Make a double-close observable without invoking undefined behavior
            // inside the test process. No real stream is closed a second time.
            note("duplicate-close\n");
            errno = EBADF;
            return EOF;
        }
        int result = next(stream);
        if (result != 0)
            _exit(125);
        note("closed-once\n");
        errno = EIO;
        return EOF;
    }
    return next(stream);
}

char *realpath(const char *path, char *out) {
    char *(*next)(const char *, char *) = symbol("realpath");
    if (interrupt_once(path, "realpath-eintr"))
        return NULL;
    return next(path, out);
}

char *__realpath_chk(const char *path, char *out, size_t length) {
    char *(*next)(const char *, char *, size_t) = symbol("__realpath_chk");
    if (interrupt_once(path, "realpath-eintr"))
        return NULL;
    return next(path, out, length);
}

int stat(const char *path, struct stat *out) {
    int (*next)(const char *, struct stat *) = symbol("stat");
    if (interrupt_once(path, "stat-eintr"))
        return -1;
    return next(path, out);
}

int stat64(const char *path, struct stat64 *out) {
    int (*next)(const char *, struct stat64 *) = symbol("stat64");
    if (interrupt_once(path, "stat-eintr"))
        return -1;
    return next(path, out);
}

int statx(int dir, const char *path, int flags, unsigned mask, struct statx *out) {
    int (*next)(int, const char *, int, unsigned, struct statx *) = symbol("statx");
    if (interrupt_once(path, "stat-eintr"))
        return -1;
    return next(dir, path, flags, mask, out);
}
