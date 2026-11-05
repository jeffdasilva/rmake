/* SPDX-License-Identifier: GPL-3.0-or-later
 * Public POSIX/C state only; no private Guile, GC, or ucontext layout. */
#ifndef RMAKE_GUILE_ENVIRONMENT_H
#define RMAKE_GUILE_ENVIRONMENT_H
#include <fenv.h>
#include <locale.h>
#include <signal.h>
struct rg_environment {
    fenv_t floating;
    sigset_t mask;
    locale_t locale;
    int error;
};
void rg_environment_capture(struct rg_environment *environment);
void rg_environment_apply(const struct rg_environment *environment);
#endif
