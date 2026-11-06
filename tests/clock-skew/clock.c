// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

/* Original deterministic Linux wall-clock interposer for the clock-skew suite. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <time.h>
#include <unistd.h>

static const char *mode(void)
{
    if (strcmp(program_invocation_short_name, "make") != 0)
        return NULL;
    const char *selected = getenv("RMAKE_TEST_CLOCK");
    return selected ? selected : "fixed";
}

static time_t seconds(const char *selected)
{
    if (access("clock-change", F_OK) == 0) {
        if (strcmp(selected, "step-forward") == 0)
            return 1600000120;
        if (strcmp(selected, "step-backward") == 0)
            return 1599999980;
    }
    return 1600000000;
}

int clock_gettime(clockid_t id, struct timespec *stamp)
{
    const char *selected = id == CLOCK_REALTIME ? mode() : NULL;
    if (selected) {
        if (strcmp(selected, "microseconds") == 0 || strcmp(selected, "seconds") == 0) {
            errno = ENOSYS;
            return -1;
        }
        if (getenv("RMAKE_TEST_CLOCK_TRACE")) {
            int fd = open("clock-events", O_WRONLY | O_APPEND | O_CLOEXEC);
            if (fd >= 0) {
                ssize_t written = write(fd, "realtime\n", 9);
                (void)written;
                close(fd);
            }
        }
        stamp->tv_sec = seconds(selected);
        stamp->tv_nsec = 400000000;
        return 0;
    }
    int (*real_clock)(clockid_t, struct timespec *) = dlsym(RTLD_NEXT, "clock_gettime");
    if (!real_clock) {
        errno = ENOSYS;
        return -1;
    }
    return real_clock(id, stamp);
}

int gettimeofday(struct timeval *stamp, void *zone)
{
    const char *selected = mode();
    if (selected && strcmp(selected, "microseconds") == 0) {
        stamp->tv_sec = seconds(selected);
        stamp->tv_usec = 400000;
        return 0;
    }
    if (selected && strcmp(selected, "seconds") == 0) {
        errno = ENOSYS;
        return -1;
    }
    int (*real_clock)(struct timeval *, void *) = dlsym(RTLD_NEXT, "gettimeofday");
    if (!real_clock) {
        errno = ENOSYS;
        return -1;
    }
    return real_clock(stamp, zone);
}

time_t time(time_t *destination)
{
    const char *selected = mode();
    if (selected && strcmp(selected, "seconds") == 0) {
        time_t whole = seconds(selected);
        if (destination)
            *destination = whole;
        return whole;
    }
    time_t (*real_clock)(time_t *) = dlsym(RTLD_NEXT, "time");
    if (!real_clock) {
        errno = ENOSYS;
        return (time_t)-1;
    }
    return real_clock(destination);
}
