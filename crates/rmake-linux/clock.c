// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva
#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <sys/time.h>
#include <time.h>

uint32_t rmake_realtime(int64_t *seconds, uint32_t *nanos)
{
    struct timespec precise;
    if (clock_gettime(CLOCK_REALTIME, &precise) == 0) {
        *seconds = precise.tv_sec;
        *nanos = precise.tv_nsec;
        return 1;
    }

    struct timeval micros;
    if (gettimeofday(&micros, NULL) == 0) {
        *seconds = micros.tv_sec;
        *nanos = micros.tv_usec * 1000;
        return 1000;
    }

    time_t whole = time(NULL);
    if (whole == (time_t)-1)
        return 0;
    *seconds = whole;
    *nanos = 0;
    return 1000000000;
}
