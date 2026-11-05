/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original public libc mask observations. */
#define _POSIX_C_SOURCE 200809L
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>

static inline void observe_mask(FILE *stream, const char *phase)
{
    sigset_t mask;
    if (sigprocmask(SIG_SETMASK, NULL, &mask) < 0)
        _Exit(126);
    if (fprintf(stream, "%s|%d%d%d\n", phase, sigismember(&mask, SIGUSR2),
                sigismember(&mask, SIGCHLD), sigismember(&mask, SIGTERM)) < 0)
        _Exit(126);
}

static inline void record_mask(const char *phase)
{
    FILE *stream = fopen("seen", "a");
    if (!stream)
        _Exit(126);
    observe_mask(stream, phase);
    if (fclose(stream) != 0)
        _Exit(126);
}
