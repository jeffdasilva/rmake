/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original executable proving the identity and order of both spooled streams. */
#define _POSIX_C_SOURCE 200809L
#include <stdio.h>
#include <sys/stat.h>
#include <unistd.h>

int main(void)
{
    struct stat out, err;
    if (fstat(STDOUT_FILENO, &out) < 0 || fstat(STDERR_FILENO, &err) < 0)
        return 126;
    if (!S_ISREG(out.st_mode) || !S_ISREG(err.st_mode))
        return 125;
    const char *kind = out.st_dev == err.st_dev && out.st_ino == err.st_ino
        ? "shared" : "separate";
    if (dprintf(STDOUT_FILENO, "%s-spool\nout-first\n", kind) < 0
        || dprintf(STDERR_FILENO, "err-middle\n") < 0
        || dprintf(STDOUT_FILENO, "out-last\n") < 0)
        return 124;
    return 0;
}
