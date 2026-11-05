/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original late process limit and post-exec rendezvous witness. */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>
#include <sys/eventfd.h>
#include <sys/resource.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

typedef char *(*make_callback)(const char *, unsigned int, char **);
extern void gmk_add_function(const char *, make_callback, unsigned int, unsigned int, unsigned int);
int plugin_is_GPL_compatible;
static atomic_int mode;

static void store(const char *path, const char *value, int flags) {
    int fd = open(path, O_WRONLY | O_CREAT | O_CLOEXEC | flags, 0644);
    if (fd < 0 || write(fd, value, strlen(value)) != (ssize_t)strlen(value) || close(fd) < 0)
        _Exit(81);
}

static char *arm(const char *name, unsigned int count, char **arguments) {
    (void)name;
    if (count != 2)
        _Exit(82);
    int limit = atoi(arguments[0]);
    int selected = strcmp(arguments[1], "exhaust") == 0 ? 2 : 1;
    struct rlimit requested = {(rlim_t)limit, (rlim_t)limit};
    struct rlimit actual;
    if ((limit != 64) || setrlimit(RLIMIT_NOFILE, &requested) != 0 ||
        getrlimit(RLIMIT_NOFILE, &actual) != 0 ||
        actual.rlim_cur != requested.rlim_cur || actual.rlim_max != requested.rlim_max)
        _Exit(83);
    int reserved = fcntl(STDIN_FILENO, F_DUPFD_CLOEXEC, 5);
    if (reserved < 5 || close(reserved) != 0)
        _Exit(84);
    store("pressure", "limit=64 private-slot=available\n", O_EXCL);
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
    if (!armed)
        return (int)syscall(SYS_eventfd2, initial, flags);

    // Only the test's late wake call waits here. The real recipe creates
    // "ready" after successful exec; no error result is fabricated.
    const struct timespec tick = {0, 1000000};
    int ready = 0;
    for (unsigned int attempt = 0; attempt < 400; ++attempt) {
        if (access("ready", F_OK) == 0) {
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
    store(trace, fd < 0 ? "recipe-ready eventfd=EMFILE\n" : "recipe-ready eventfd=opened\n", O_APPEND);
    errno = saved;
    return fd;
}
