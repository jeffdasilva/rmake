/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Descriptor-free Linux recipe launch via the public libc spawn API. */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <spawn.h>
#include <stdint.h>
#include <stdio.h>
#include <sys/resource.h>
#include <sys/stat.h>

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

static int glibc_at_least(unsigned required_minor)
{
    /* 2.24 used an error pipe. In 2.26, a child killed before recording an
       exec result could trip a libc assertion. Use the 2.27+ behavior. */
    unsigned major, minor;
    return sscanf(gnu_get_libc_version(), "%u.%u", &major, &minor) == 2
        && (major > 2 || (major == 2 && minor >= required_minor));
}
#endif

int rmake_compact_pipe_budget(unsigned minimum)
{
#ifdef __GLIBC__
    struct rlimit limit;
    return getrlimit(RLIMIT_NOFILE, &limit) == 0
        && limit.rlim_cur < minimum && glibc_at_least(29);
#else
    (void)minimum;
    return 0;
#endif
}

/* A raw inherited slot is not a File. Check its actual pipe identity and
   direction before the Rust protocol acts on it. */
int rmake_pipe_identity(int fd, int reader, uint64_t identity[2])
{
    struct stat metadata;
    int result;
    do {
        result = fstat(fd, &metadata);
    } while (result < 0 && errno == EINTR);
    if (result < 0)
        return errno;
    int mode;
    do {
        mode = fcntl(fd, F_GETFL);
    } while (mode < 0 && errno == EINTR);
    if (mode < 0)
        return errno;
    int access = mode & O_ACCMODE;
    if (!S_ISFIFO(metadata.st_mode) || (mode & O_PATH)
        || (access != O_RDWR && access != (reader ? O_RDONLY : O_WRONLY)))
        return EBADF;
    identity[0] = (uint64_t)metadata.st_dev;
    identity[1] = (uint64_t)metadata.st_ino;
    return 0;
}

/* -1 means the backend was not selected and no child was started. A positive
   result is the actual spawn/exec errno. Only a zero result transfers one PID.
   descriptors names stdin/out/err and an optional inherited protocol pair. */
int rmake_spawn_inherited(const char *path, char *const argv[],
                          char *const envp[], int preserve_pipe_ignore,
                          const int descriptors[5], int *pid)
{
#ifdef __GLIBC__
    int recursive_pipe = descriptors[3] >= 0;
    /* From 2.29, a spawn dup2 action with equal source/destination clears
       CLOEXEC in the child without opening or modifying the parent's pair. */
    if (!glibc_at_least(recursive_pipe ? 29 : 27))
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
    posix_spawn_file_actions_t actions;
    posix_spawn_file_actions_t *configured = NULL;
    if (!error && (descriptors[0] >= 0 || descriptors[1] >= 0
                   || descriptors[2] >= 0 || recursive_pipe)) {
        error = posix_spawn_file_actions_init(&actions);
        if (!error)
            configured = &actions;
        for (int index = 0; !error && index < 5; ++index) {
            int fd = descriptors[index];
            if (fd >= 0)
                error = posix_spawn_file_actions_adddup2(&actions, fd, index < 3 ? index : fd);
        }
    }
    if (!error)
        error = posix_spawn(&child, path, configured, &attributes, argv, envp);
    if (configured)
        posix_spawn_file_actions_destroy(configured);
    posix_spawnattr_destroy(&attributes);
    if (!error)
        *pid = child;
    return error;
#else
    (void)path;
    (void)argv;
    (void)envp;
    (void)preserve_pipe_ignore;
    (void)descriptors;
    (void)pid;
    return -1;
#endif
}
