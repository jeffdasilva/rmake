// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva
// Development-only glibc allocation sampler; never linked into rmake.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <execinfo.h>
#include <fcntl.h>
#include <link.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

// This helper intentionally targets the pinned glibc development profile.
// Direct allocator entry points avoid recursive dlsym initialization.
extern void *__libc_malloc(size_t);
extern void *__libc_calloc(size_t, size_t);
extern void *__libc_realloc(void *, size_t);
extern void __libc_free(void *);

static _Atomic unsigned long calls, bytes, frees;
static _Atomic int ready, write_failed;
static int trace_fd = -1;
static uintptr_t image_base;
static _Thread_local int recording;

static int find_image(struct dl_phdr_info *info, size_t size, void *data) {
    (void)size;
    (void)data;
    if (!info->dlpi_name[0]) image_base = info->dlpi_addr;
    return 0;
}

static void append_record(const char *line, size_t length) {
    ssize_t written;
    do { written = write(trace_fd, line, length); } while (written < 0 && errno == EINTR);
    if (written < 0 || (size_t)written != length) atomic_store(&write_failed, 1);
}

__attribute__((constructor)) static void start(void) {
    const char *path = getenv("RMAKE_ALLOC_TRACE");
    if (!path) return;
    trace_fd = open(path, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0600);
    dl_iterate_phdr(find_image, NULL);
    atomic_store(&ready, trace_fd >= 0);
}

static void sample(size_t size) {
    if (!atomic_load(&ready) || recording) return;
    recording = 1;
    unsigned long call = atomic_fetch_add(&calls, 1) + 1;
    atomic_fetch_add(&bytes, size);
    if (call % 512 == 0 || size >= 65536) {
        void *frames[20];
        int count = backtrace(frames, 20);
        char line[2048];
        int length = snprintf(line, sizeof line, "sample %lu %zu", call, size);
        for (int index = 1; index < count; ++index) {
            Dl_info info;
            if (dladdr(frames[index], &info) && (uintptr_t)info.dli_fbase == image_base) {
                // Twenty machine addresses and two decimal counters fit the
                // fixed buffer, independent of filenames or symbol text.
                length += snprintf(line + length, sizeof line - length, " 0x%lx",
                                   (unsigned long)((uintptr_t)frames[index] - image_base));
            }
        }
        line[length++] = '\n';
        append_record(line, (size_t)length);
    }
    recording = 0;
}

void *malloc(size_t size) {
    void *pointer = __libc_malloc(size);
    sample(size);
    return pointer;
}

void *calloc(size_t count, size_t size) {
    void *pointer = __libc_calloc(count, size);
    if (!size || count <= SIZE_MAX / size) sample(count * size);
    return pointer;
}

void *realloc(void *pointer, size_t size) {
    void *replacement = __libc_realloc(pointer, size);
    sample(size);
    return replacement;
}

void free(void *pointer) {
    if (atomic_load(&ready) && !recording) atomic_fetch_add(&frees, 1);
    __libc_free(pointer);
}

__attribute__((destructor)) static void finish(void) {
    atomic_store(&ready, 0);
    if (trace_fd < 0) return;
    if (!atomic_load(&write_failed)) {
        char line[200];
        int length = snprintf(line, sizeof line,
                              "summary calls=%lu requested_bytes=%lu frees=%lu\n",
                              atomic_load(&calls), atomic_load(&bytes), atomic_load(&frees));
        append_record(line, (size_t)length);
    }
    close(trace_fd);
}
