/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Qualify forwarding without importing a nonnull caller assumption. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <sys/stat.h>
int main(void) {
  typedef int (*function)(int, const char *, int, unsigned, struct statx *);
  function operation = (function)dlsym(RTLD_DEFAULT, "statx");
  if (!operation) return 77;
  struct statx value;
  errno = 0;
  int result = operation(AT_FDCWD, NULL, AT_EMPTY_PATH, STATX_BASIC_STATS, &value);
  int error = result < 0 ? errno : 0;
  printf("statx-null=%d:%d\n", result, error);
  return 0;
}
