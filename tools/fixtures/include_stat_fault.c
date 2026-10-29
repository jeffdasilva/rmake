/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original selective metadata fault observer; trusted fixtures only. */
#define _GNU_SOURCE
#define _LARGEFILE64_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <signal.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

static unsigned target_calls;
static int inject(const char *path, const char *operation) {
  if (!path || strcmp(path, "inc")) return 0;
  const char *raw = getenv("RMAKE_INCLUDE_STAT_INTERRUPTS");
  unsigned faults = raw ? (unsigned)strtoul(raw, 0, 10) : 0;
  int fail = ++target_calls <= faults;
  const char *trace = getenv("RMAKE_INCLUDE_STAT_TRACE");
  if (trace) {
    int fd = open(trace, O_WRONLY | O_CREAT | O_APPEND, 0600);
    if (fd >= 0) {
      char line[128];
      int n = snprintf(line, sizeof line, "%s %s %u\n", fail ? "eintr" : "real", operation, target_calls);
      if (n > 0 && n < (int)sizeof line) {
        size_t offset = 0;
        while (offset < (size_t)n) {
          ssize_t written = write(fd, line + offset, (size_t)n - offset);
          if (written < 0 && errno == EINTR) continue;
          if (written <= 0) break;
          offset += (size_t)written;
        }
      }
      (void)close(fd);
    }
  }
  if (fail) {
    const char *number = getenv("RMAKE_INCLUDE_STAT_SIGNAL");
    if (number && *number) (void)raise(atoi(number));
    errno = EINTR;
  }
  return fail;
}

/* Separate C identifiers preserve null forwarding without inheriting libc
 * declaration nonnull assumptions during interposer optimization. */
int observe_stat(const char *path, struct stat *value) __asm__("stat");
int observe_stat(const char *path, struct stat *value) {
  typedef int (*function)(const char *, struct stat *);
  static function next;
  if (!next) next = (function)dlsym(RTLD_NEXT, "stat");
  if (inject(path, "stat")) return -1;
  return next(path, value);
}
int observe_stat64(const char *path, struct stat64 *value) __asm__("stat64");
int observe_stat64(const char *path, struct stat64 *value) {
  typedef int (*function)(const char *, struct stat64 *);
  static function next;
  if (!next) next = (function)dlsym(RTLD_NEXT, "stat64");
  if (inject(path, "stat64")) return -1;
  return next(path, value);
}
int observe_statx(int fd, const char *path, int flags, unsigned mask, struct statx *value) __asm__("statx");
int observe_statx(int fd, const char *path, int flags, unsigned mask, struct statx *value) {
  typedef int (*function)(int, const char *, int, unsigned, struct statx *);
  static function next;
  if (!next) next = (function)dlsym(RTLD_NEXT, "statx");
  if (inject(path, "statx")) return -1;
  if (!next) { errno = ENOSYS; return -1; }
  return next(fd, path, flags, mask, value);
}
