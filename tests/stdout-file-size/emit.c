/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original control for child-only stdout pressure. */
#include <unistd.h>
#include <string.h>

int main(int argc, char **argv)
{
    int newline = argc > 1 && !strcmp(argv[1], "newline");
    int error = argc > 1 && !strcmp(argv[1], "stderr");
    return write(error ? STDERR_FILENO : STDOUT_FILENO,
                 newline ? "ABCD\nFGH" : "ABCDEFGH", 8) == 5 ? 0 : 91;
}
