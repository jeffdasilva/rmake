/* SPDX-License-Identifier: GPL-3.0-or-later */
/* Copyright 2026 Jeff DaSilva */

#include <errno.h>
#include <fcntl.h>
#include <unistd.h>

int main(void) {
    static const char err[] = "child-stderr\n";
    static const char out[] = "capture\n";
    if (fcntl(0, F_GETFD) >= 0 || errno != EBADF)
        return 26;
    if (write(2, err, sizeof err - 1) != sizeof err - 1)
        return 27;
    if (fcntl(1, F_GETFD) < 0)
        return errno == EBADF ? 23 : 25;
    return write(1, out, sizeof out - 1) == sizeof out - 1 ? 0 : 28;
}
