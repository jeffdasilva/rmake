/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original late process limit and post-exec rendezvous witness. */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/filter.h>
#include <linux/seccomp.h>
#include <stddef.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>
#include <sys/eventfd.h>
#include <sys/resource.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

typedef char *(*make_callback)(const char *, unsigned int, char **);
extern void gmk_add_function(const char *, make_callback, unsigned int, unsigned int, unsigned int);
int plugin_is_GPL_compatible;
static atomic_int mode;
static atomic_int attempts;

static void store(const char *path, const char *value, int flags) {
    int fd = open(path, O_WRONLY | O_CREAT | O_CLOEXEC | flags, 0644);
    if (fd < 0 || write(fd, value, strlen(value)) != (ssize_t)strlen(value) || close(fd) < 0)
        _Exit(81);
}

static void block_pidfd_open(void) {
    struct sock_filter rules[] = {
        BPF_STMT(BPF_LD | BPF_W | BPF_ABS, offsetof(struct seccomp_data, nr)),
        BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, SYS_pidfd_open, 0, 1),
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | ENOSYS),
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW),
    };
    struct sock_fprog program = {sizeof rules / sizeof *rules, rules};
    if (prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 ||
        syscall(SYS_seccomp, SECCOMP_SET_MODE_FILTER, 0, &program) != 0 ||
        syscall(SYS_pidfd_open, getpid(), 0) != -1 || errno != ENOSYS)
        _Exit(91);
}

static char *arm(const char *name, unsigned int count, char **arguments) {
    (void)name;
    if (count != 2)
        _Exit(82);
    int limit = atoi(arguments[0]);
    int pidless = strcmp(arguments[1], "exhaust-pidless") == 0;
    int selected = strcmp(arguments[1], "normal") == 0 ? 1 : 2;
    struct rlimit requested = {(rlim_t)limit, (rlim_t)limit};
    struct rlimit actual;
    if ((limit != 64) || setrlimit(RLIMIT_NOFILE, &requested) != 0 ||
        getrlimit(RLIMIT_NOFILE, &actual) != 0 ||
        actual.rlim_cur != requested.rlim_cur || actual.rlim_max != requested.rlim_max)
        _Exit(83);
    int reserved = fcntl(STDIN_FILENO, F_DUPFD_CLOEXEC, 5);
    if (reserved < 5 || close(reserved) != 0)
        _Exit(84);
    if (pidless)
        block_pidfd_open();
    store("pressure", pidless ? "limit=64 pidfd=ENOSYS\n" : "limit=64\n", O_EXCL);
    atomic_store_explicit(&mode, selected, memory_order_release);
    return NULL;
}

int pressure_gmk_setup(const void *location) {
    (void)location;
    gmk_add_function("fdpressure", arm, 2, 2, 0);
    return 1;
}

int eventfd(unsigned int initial, int flags) {
    int armed = atomic_load_explicit(&mode, memory_order_acquire);
    if (!armed || access("callback.done", F_OK) == 0)
        return (int)syscall(SYS_eventfd2, initial, flags);

    // Both recipe processes publish proof of exec before the real syscall.
    // Any second wait allocation before callback completion is retained.
    const struct timespec tick = {0, 1000000};
    int ready = 0;
    for (unsigned int attempt = 0; attempt < 400; ++attempt) {
        if (access("alpha.ready", F_OK) == 0 && access("beta.ready", F_OK) == 0) {
            ready = 1;
            break;
        }
        nanosleep(&tick, NULL);
    }
    if (!ready)
        _Exit(86);
    int occupied[64];
    unsigned int count = 0;
    if (armed == 2) {
        for (; count < sizeof occupied / sizeof *occupied; ++count) {
            int descriptor = open("/dev/null", O_RDONLY | O_CLOEXEC);
            if (descriptor < 0) {
                if (errno != EMFILE)
                    _Exit(87);
                break;
            }
            occupied[count] = descriptor;
        }
        if (count == 0 || count == sizeof occupied / sizeof *occupied)
            _Exit(87);
    }
    int fd = (int)syscall(SYS_eventfd2, initial, flags);
    int saved = errno;
    if (armed == 2) {
        for (unsigned int index = 0; index < count; ++index)
            if (close(occupied[index]) != 0)
                _Exit(88);
        if (fd != -1 || saved != EMFILE)
            _Exit(88);
    } else if (fd < 0) {
        _Exit(89);
    }
    const char *trace = getenv("PRESSURE_OBSERVER");
    if (!trace)
        _Exit(90);
    if (atomic_fetch_add_explicit(&attempts, 1, memory_order_relaxed))
        store(trace, "premature rearm\n", O_APPEND);
    else
        store(trace, fd < 0 ? "both-executed eventfd=EMFILE\n" : "both-executed eventfd=opened\n", O_APPEND);
    errno = saved;
    return fd;
}
