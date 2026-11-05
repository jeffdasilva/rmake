/* SPDX-License-Identifier: GPL-3.0-or-later */
/* Copyright 2026 Jeff DaSilva */

#include <errno.h>
#include <fcntl.h>
#include <string.h>
#include <unistd.h>

int main(int argc, char **argv) {
    if (argc != 2)
        return 28;
    int stdin_open = fcntl(0, F_GETFD) >= 0;
    if (stdin_open != (strcmp(argv[1], "stdout-only") == 0) || (!stdin_open && errno != EBADF))
        return 29;
    if (fcntl(1, F_GETFD) < 0)
        return errno == EBADF ? 23 : 25;
    static const char captured[] = "capture\n";
    return write(1, captured, sizeof captured - 1) == sizeof captured - 1 ? 0 : 27;
}
