/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Descriptor-free Linux recipe launch via the public libc spawn API. */
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <signal.h>
#include <spawn.h>
#include <stdio.h>

/* The make reader has only SIGCHLD blocked. An exec child starts unblocked.
   All operations here are async-signal-safe for use after fork. */
int rmake_set_execution_mask(int reader)
{
    sigset_t mask;
    if (sigemptyset(&mask) < 0 || (reader && sigaddset(&mask, SIGCHLD) < 0)
        || sigprocmask(SIG_SETMASK, &mask, NULL) < 0)
        return errno;
    return 0;
}

#ifdef __GLIBC__
#include <gnu/libc-version.h>

static int spawn_returns_exec_errno(void)
{
    /* 2.24 used an error pipe. In 2.26, a child killed before recording an
       exec result could trip a libc assertion. Use the 2.27+ behavior. */
    unsigned major, minor;
    return sscanf(gnu_get_libc_version(), "%u.%u", &major, &minor) == 2
        && (major > 2 || (major == 2 && minor >= 27));
}
#endif

/* -1 means the backend was not selected and no child was started. A positive
   result is the actual spawn/exec errno. Only a zero result transfers one PID. */
int rmake_spawn_inherited(const char *path, char *const argv[],
                          char *const envp[], int preserve_pipe_ignore, int *pid)
{
#ifdef __GLIBC__
    if (!spawn_returns_exec_errno())
        return -1;
    if (preserve_pipe_ignore) {
        struct sigaction current;
        if (sigaction(SIGPIPE, NULL, &current) < 0)
            return errno;
        /* POSIX spawn can reset a signal, but cannot install SIG_IGN. If a
           native module replaced the inherited ignore, use the normal hook. */
        if (current.sa_handler != SIG_IGN)
            return -1;
    }
    pid_t child;
    posix_spawnattr_t attributes;
    int error = posix_spawnattr_init(&attributes);
    if (error)
        return error;
    sigset_t empty;
    error = sigemptyset(&empty) < 0 ? errno : posix_spawnattr_setsigmask(&attributes, &empty);
    short flags = POSIX_SPAWN_SETSIGMASK;
    if (!error && !preserve_pipe_ignore) {
        sigset_t defaults;
        if (sigemptyset(&defaults) < 0 || sigaddset(&defaults, SIGPIPE) < 0)
            error = errno;
        if (!error)
            error = posix_spawnattr_setsigdefault(&attributes, &defaults);
        flags |= POSIX_SPAWN_SETSIGDEF;
    }
    if (!error)
        error = posix_spawnattr_setflags(&attributes, flags);
    if (!error)
        error = posix_spawn(&child, path, NULL, &attributes, argv, envp);
    posix_spawnattr_destroy(&attributes);
    if (!error)
        *pid = child;
    return error;
#else
    (void)path;
    (void)argv;
    (void)envp;
    (void)preserve_pipe_ignore;
    (void)pid;
    return -1;
#endif
}
