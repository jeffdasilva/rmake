/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Owned Linux process-replacement environment; no make implementation code. */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

struct restart_labels {
    char level[32];
    char first[32];
    char following[32];
};

static const char *entry_bytes(const char *entry,
                               const struct restart_labels *labels,
                               int *found_restart)
{
    if (!strncmp(entry, "MAKELEVEL=", sizeof("MAKELEVEL=") - 1))
        return labels->level;
    if (!strncmp(entry, "MAKE_RESTARTS=", sizeof("MAKE_RESTARTS=") - 1)) {
        const char *result = *found_restart ? labels->following : labels->first;
        *found_restart = 1;
        return result;
    }
    return entry;
}

struct environment_image {
    char **slots;
    size_t capacity;
    size_t used;
    char *arena;
    size_t remaining;
};

static int append_entry(struct environment_image *image, const char *value)
{
    size_t length = strlen(value);
    if (image->used >= image->capacity - 1 || length >= image->remaining)
        return E2BIG;
    ++length;
    image->slots[image->used++] = image->arena;
    memcpy(image->arena, value, length);
    image->arena += length;
    image->remaining -= length;
    return 0;
}

static int copy_live_environment(const struct restart_labels *labels,
                                 uint32_t count, char ***result)
{
    /* One owned allocation: raw slots followed by their byte storage. The
       extra slot/span covers an absent restart counter. Never edit environ
       or hand a borrowed environment string across the Rust boundary. */
    size_t slots = 2;
    size_t bytes = strlen(labels->first) + 1;
    int found = 0;
    for (char *const *slot = environ; slot && *slot; ++slot) {
        size_t length = strlen(entry_bytes(*slot, labels, &found));
        if (slots == SIZE_MAX / sizeof(char *) || length >= SIZE_MAX - bytes)
            return EOVERFLOW;
        ++slots;
        bytes += length + 1;
    }
    size_t pointer_bytes = slots * sizeof(char *);
    if (bytes > SIZE_MAX - pointer_bytes)
        return EOVERFLOW;
    char **storage = malloc(pointer_bytes + bytes);
    if (!storage)
        return ENOMEM;

    struct environment_image image = {
        storage, slots, 0, (char *)storage + pointer_bytes, bytes
    };
    int failure = 0;
    found = 0;
    for (char *const *slot = environ; slot && *slot; ++slot) {
        failure = append_entry(&image, entry_bytes(*slot, labels, &found));
        if (failure)
            break;
    }
    if (!failure && !found && count)
        failure = append_entry(&image, labels->first);
    if (failure) {
        free(storage);
        return failure;
    }
    image.slots[image.used] = NULL;
    *result = storage;
    return 0;
}

static int expose_descriptor(int descriptor)
{
    if (descriptor < 0)
        return 0;
    for (;;) {
        if (fcntl(descriptor, F_SETFD, 0) >= 0)
            return 0;
        if (errno != EINTR)
            return errno;
    }
}

/* Called only by the serialized process owner, after makefile jobs finish.
   POSIX native extensions may update the process environment at their normal
   synchronous callbacks, but must not mutate it concurrently with a host
   access. This is the same environment contract as initial import/getenv.
   execvpe searches the live PATH; the replacement vector only changes the
   counter slots and leaves the global environment untouched on failure. */
int rmake_exec_live_restart(char *const arguments[], uint32_t count,
                            uint32_t level, int traced, int reader, int writer)
{
    struct restart_labels labels;
    const char *marker = traced ? "-" : "";
    int level_length = snprintf(labels.level, sizeof(labels.level),
                                "MAKELEVEL=%" PRIu32, level);
    int first_length = snprintf(labels.first, sizeof(labels.first),
                                "MAKE_RESTARTS=%s%" PRIu32, marker, count);
    int later_length = snprintf(labels.following, sizeof(labels.following),
                                "MAKE_RESTARTS=%s0", marker);
    if (level_length < 0 || (size_t)level_length >= sizeof(labels.level)
        || first_length < 0 || (size_t)first_length >= sizeof(labels.first)
        || later_length < 0 || (size_t)later_length >= sizeof(labels.following))
        return EOVERFLOW;

    char **environment;
    int failure = copy_live_environment(&labels, count, &environment);
    if (failure)
        return failure;
    failure = expose_descriptor(reader);
    if (!failure)
        failure = expose_descriptor(writer);
    if (!failure) {
        execvpe(arguments[0], arguments, environment);
        failure = errno;
    }
    free(environment);
    return failure;
}
