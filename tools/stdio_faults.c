// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva.
// Original test-only Linux/glibc interposer. No GNU Make implementation copied.
// Only the literal fixture basename effect-probe is faulted. The journal proves
// that a requested fault happened; it preserves errno and uses no stdio calls.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdarg.h>
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

static int symlink_fault(const char *path, const char *operation) {
    if (getenv("RMAKE_ARCHIVE_ARMED") && access("arm.marker", F_OK) != 0) return 0;
    const char *mode = getenv("RMAKE_STDIO_FAULT");
    if (!selected(path) || !mode || strncmp(mode, "symlink-", 8) || interrupted)
        return 0;
    const char *kind = mode + 8;
    size_t length = strlen(operation);
    if (strncmp(kind, operation, length) || kind[length] != '-') return 0;
    kind += length + 1;
    int error = !strcmp(kind, "eintr") ? EINTR
        : !strcmp(kind, "eio") ? EIO
        : !strcmp(kind, "eacces") ? EACCES
        : !strcmp(kind, "enoent") ? ENOENT
        : !strcmp(kind, "enotdir") ? ENOTDIR : 0;
    if (!error) return 0;
    interrupted = 1;
    note("symlink-fault\n");
    errno = error;
    return 1;
}

// Archive read/seek probes select real descriptors by their fixture inode path.
// Read faults occur at byte offsets, independent of read sizes or call counts.
static int selected_descriptor(int descriptor) {
    char link[64], path[4096];
    int length = snprintf(link, sizeof link, "/proc/self/fd/%d", descriptor);
    if (length < 0 || (size_t)length >= sizeof link) return 0;
    ssize_t (*next)(const char *, char *, size_t) = symbol("readlink");
    ssize_t size = next(link, path, sizeof path - 1);
    if (size < 0) return 0;
    path[size] = 0;
    return selected(path);
}

static int archive_seek_fault(int descriptor, off64_t offset, int whence) {
    const char *mode = getenv("RMAKE_STDIO_FAULT");
    if (getenv("RMAKE_ARCHIVE_ARMED") && access("arm.marker", F_OK) != 0) return 0;
    if (interrupted || !mode || strncmp(mode, "archive-seek-", 13)
        || offset != 8 || whence != SEEK_SET || !selected_descriptor(descriptor)) return 0;
    const char *kind = mode + 13;
    int error = !strcmp(kind, "eintr") ? EINTR : !strcmp(kind, "eio") ? EIO : 0;
    if (!error) return 0;
    interrupted = 1;
    note("archive-seek-fault\n");
    errno = error;
    return 1;
}

off_t lseek(int descriptor, off_t offset, int whence) {
    off_t (*next)(int, off_t, int) = symbol("lseek");
    if (archive_seek_fault(descriptor, offset, whence)) return -1;
    return next(descriptor, offset, whence);
}

off64_t lseek64(int descriptor, off64_t offset, int whence) {
    off64_t (*next)(int, off64_t, int) = symbol("lseek64");
    if (archive_seek_fault(descriptor, offset, whence)) return -1;
    return next(descriptor, offset, whence);
}

ssize_t read(int descriptor, void *buffer, size_t length) {
    ssize_t (*next)(int, void *, size_t) = symbol("read");
    const char *mode = getenv("RMAKE_STDIO_FAULT");
    if ((!getenv("RMAKE_ARCHIVE_ARMED") || access("arm.marker", F_OK) == 0)
        && !interrupted && mode && !strncmp(mode, "archive-read-", 13)
        && selected_descriptor(descriptor)) {
        off64_t (*position)(int, off64_t, int) = symbol("lseek64");
        off64_t offset = position(descriptor, 0, SEEK_CUR);
        const char *phase = mode + 13, *kind = NULL;
        off64_t boundary = -1;
        if (!strncmp(phase, "signature-", 10)) { boundary = 0; kind = phase + 10; }
        if (!strncmp(phase, "header-", 7)) { boundary = 8; kind = phase + 7; }
        if (!strncmp(phase, "next-", 5)) { boundary = 76; kind = phase + 5; }
        if (boundary >= 0 && offset >= 0 && offset < boundary
            && length > (size_t)(boundary - offset)) length = (size_t)(boundary - offset);
        if (kind && offset == boundary) {
            interrupted = 1;
            note("archive-read-fault\n");
            if (!strcmp(kind, "eintr")) { errno = EINTR; return -1; }
            if (!strcmp(kind, "eio")) { errno = EIO; return -1; }
            if (!strcmp(kind, "eof")) return 0;
            if (!strcmp(kind, "short") && length > 3) length = 3;
        }
    }
    return next(descriptor, buffer, length);
}

static void archive_write_replacement(const char *path, int flags) {
    if (!interrupted && selected(path) && mode_is("archive-touch-replace")
        && (flags & O_ACCMODE) == O_RDWR && access("arm.marker", F_OK) == 0) {
        interrupted = 1;
        if (rename("replacement.a", path) != 0) _exit(123);
        note("archive-replaced\n");
    }
}

int open(const char *path, int flags, ...) {
    int (*next)(const char *, int, ...) = symbol("open");
    mode_t mode = 0;
    if ((flags & O_CREAT) || ((flags & O_TMPFILE) == O_TMPFILE)) {
        va_list args;
        va_start(args, flags);
        mode = va_arg(args, mode_t);
        va_end(args);
    }
    if (interrupt_once(path, "archive-open-eintr")) return -1;
    archive_write_replacement(path, flags);
    return next(path, flags, mode);
}

int open64(const char *path, int flags, ...) {
    int (*next)(const char *, int, ...) = symbol("open64");
    mode_t mode = 0;
    if ((flags & O_CREAT) || ((flags & O_TMPFILE) == O_TMPFILE)) {
        va_list args;
        va_start(args, flags);
        mode = va_arg(args, mode_t);
        va_end(args);
    }
    if (interrupt_once(path, "archive-open-eintr")) return -1;
    archive_write_replacement(path, flags);
    return next(path, flags, mode);
}

// Directory probes are test-only and restricted to d / d<number>. Track
// actual libc streams; never return synthesized entries or rule decisions.
struct watched_directory {
    DIR *stream;
    unsigned active, eof, seen_name;
    char name[32];
};
static struct watched_directory watched[32];
static unsigned watched_count;

static int directory_mode(void) {
    const char *mode = getenv("RMAKE_STDIO_FAULT");
    return mode && strncmp(mode, "directory-", 10) == 0;
}

static int selected_directory(const char *path) {
    const char *name = strrchr(path, '/');
    name = name ? name + 1 : path;
    if (*name++ != 'd') return 0;
    while (*name >= '0' && *name <= '9') ++name;
    return *name == 0;
}

static struct watched_directory *directory_record(DIR *stream) {
    for (unsigned index = 0; index < watched_count; ++index)
        if (watched[index].stream == stream && watched[index].active)
            return &watched[index];
    return NULL;
}

DIR *opendir(const char *path) {
    DIR *(*next)(const char *) = symbol("opendir");
    int selected = directory_mode() && selected_directory(path);
    if (selected && mode_is("directory-open-eintr") && !interrupted) {
        interrupted = 1;
        note("interrupted\n");
        errno = EINTR;
        return NULL;
    }
    if (selected && mode_is("directory-open-eacces")) {
        note("open-denied\n");
        errno = EACCES;
        return NULL;
    }
    DIR *stream = next(path);
    if (selected && stream) {
        unsigned index;
        for (index = 0; index < watched_count; ++index)
            if (!watched[index].active) break;
        if (index == watched_count) {
            if (watched_count == 32) _exit(128);
            ++watched_count;
        }
        struct watched_directory *entry = &watched[index];
        memset(entry, 0, sizeof(*entry));
        entry->stream = stream;
        entry->active = 1;
        const char *name = strrchr(path, '/');
        snprintf(entry->name, sizeof(entry->name), "%s", name ? name + 1 : path);
    }
    return stream;
}

static int directory_read_fault(struct watched_directory *entry) {
    if (!entry || interrupted) return 0;
    if (mode_is("directory-read-eintr")) {
        interrupted = 1;
        note("interrupted\n");
        errno = EINTR;
        return 1;
    }
    if (mode_is("directory-read-eio")
        || (mode_is("directory-read-tail-eio") && entry->seen_name)) {
        interrupted = 1;
        note("read-fault\n");
        errno = EIO;
        return 1;
    }
    return 0;
}

struct dirent *readdir(DIR *stream) {
    struct dirent *(*next)(DIR *) = symbol("readdir");
    struct watched_directory *entry = directory_record(stream);
    if (directory_read_fault(entry)) return NULL;
    struct dirent *result = next(stream);
    if (entry) {
        if (!result && errno == 0) entry->eof = 1;
        if (result && strcmp(result->d_name, ".") && strcmp(result->d_name, ".."))
            entry->seen_name = 1;
    }
    return result;
}

struct dirent64 *readdir64(DIR *stream) {
    struct dirent64 *(*next)(DIR *) = symbol("readdir64");
    struct watched_directory *entry = directory_record(stream);
    if (directory_read_fault(entry)) return NULL;
    struct dirent64 *result = next(stream);
    if (entry) {
        if (!result && errno == 0) entry->eof = 1;
        if (result && strcmp(result->d_name, ".") && strcmp(result->d_name, ".."))
            entry->seen_name = 1;
    }
    return result;
}

int closedir(DIR *stream) {
    int (*next)(DIR *) = symbol("closedir");
    struct watched_directory *entry = directory_record(stream);
    if (entry) entry->active = 0;
    int result = next(stream);
    if (entry && mode_is("directory-close-eio")) {
        note("closed-once\n");
        errno = EIO;
        return -1;
    }
    return result;
}

static void directory_proof(void) {
    if (mode_is("directory-limit")) {
        unsigned active = 0, complete = 0;
        for (unsigned index = 0; index < watched_count; ++index) {
            active += watched[index].active;
            if (!strcmp(watched[index].name, "d9")) complete = watched[index].eof;
        }
        char text[64];
        snprintf(text, sizeof(text), "open=%u last=%s\n", active, complete ? "complete" : "partial");
        note(text);
    } else {
        const char *state = "unseen\n";
        for (unsigned index = 0; index < watched_count; ++index) {
            if (!strcmp(watched[index].name, "d"))
                state = watched[index].eof ? "complete\n" : "partial\n";
        }
        note(state);
    }
}

static void external_mutation(void) {
    if (mode_is("directory-partial") || mode_is("directory-complete") || mode_is("directory-limit")) {
        directory_proof();
        return;
    }
    const char *file = "b.t";
    if (mode_is("wildcard-files")) {
        if (unlink("a.t")) _exit(126);
    } else if (mode_is("wildcard-missing")) {
        if (mkdir("missing", 0755)) _exit(126);
        file = "missing/b.t";
    } else if (mode_is("wildcard-alias")) {
        if (unlink("d/a.t")) _exit(126);
        file = "d/b.t";
    } else if (mode_is("wildcard-replaced")) {
        if (rename("d", "old") || mkdir("d", 0755)) _exit(126);
        file = "d/b.t";
    } else return;
    int fd = open(file, O_WRONLY | O_CREAT | O_EXCL, 0644);
    if (fd < 0 || write(fd, "new\n", 4) != 4 || close(fd)) _exit(127);
    note("external-mutation\n");
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
    if (selected(path) && mode[0] == 'r' && !interrupted) {
        external_mutation();
        interrupted = 1;
    }
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
    if (symlink_fault(path, "stat")) return -1;
    return next(path, out);
}

int stat64(const char *path, struct stat64 *out) {
    int (*next)(const char *, struct stat64 *) = symbol("stat64");
    if (interrupt_once(path, "stat-eintr"))
        return -1;
    if (symlink_fault(path, "stat")) return -1;
    return next(path, out);
}

int lstat(const char *path, struct stat *out) {
    int (*next)(const char *, struct stat *) = symbol("lstat");
    if (interrupt_once(path, "stat-eintr")) return -1;
    if (symlink_fault(path, "lstat")) return -1;
    return next(path, out);
}

int lstat64(const char *path, struct stat64 *out) {
    int (*next)(const char *, struct stat64 *) = symbol("lstat64");
    if (interrupt_once(path, "stat-eintr")) return -1;
    if (symlink_fault(path, "lstat")) return -1;
    return next(path, out);
}

int statx(int dir, const char *path, int flags, unsigned mask, struct statx *out) {
    int (*next)(int, const char *, int, unsigned, struct statx *) = symbol("statx");
    if (interrupt_once(path, "stat-eintr"))
        return -1;
    if (symlink_fault(path, (flags & AT_SYMLINK_NOFOLLOW) ? "lstat" : "stat")) return -1;
    return next(dir, path, flags, mask, out);
}

ssize_t readlink(const char *path, char *out, size_t length) {
    ssize_t (*next)(const char *, char *, size_t) = symbol("readlink");
    if (symlink_fault(path, "readlink")) return -1;
    return next(path, out, length);
}
