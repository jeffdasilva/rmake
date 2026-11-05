/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 */
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <string.h>

/* Use the thread-safe POSIX declaration through the system headers. */
int rm_source_strerror(int code, char *buffer, size_t capacity) {
    int saved = errno;
    int status = strerror_r(code, buffer, capacity);
    errno = saved;
    if (status == 0)
        return 0;
    return status == ERANGE ? 1 : -1;
}
