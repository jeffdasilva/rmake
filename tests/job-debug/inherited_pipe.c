// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva
// Original test witness for a live inherited controller with fixed descriptors.
#define _GNU_SOURCE
#include <fcntl.h>
#include <unistd.h>

static void __attribute__((constructor))
make_controller(void)
{
    int pipe[2];
    if (pipe2(pipe, O_CLOEXEC) != 0 || pipe[0] >= 20 || pipe[1] >= 20)
        _exit(110);
    if (dup2(pipe[0], 20) != 20 || dup2(pipe[1], 21) != 21)
        _exit(111);
    if (close(pipe[0]) != 0 || close(pipe[1]) != 0 || write(21, "+", 1) != 1)
        _exit(112);
}
