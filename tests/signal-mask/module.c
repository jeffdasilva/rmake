/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original module using only the public native function ABI. */
#include "mask_bits.h"
#include <fcntl.h>
#include <string.h>
#include <unistd.h>

struct Place { const char *file; unsigned long line; };
typedef char *(*Callback)(const char *, unsigned int, char **);
extern void gmk_add_function(const char *, Callback, unsigned int, unsigned int, unsigned int);
extern char *gmk_alloc(unsigned int);
int plugin_is_GPL_compatible = 1;

__attribute__((constructor))
static void loaded(void)
{
    record_mask("load-ctor");
}

__attribute__((destructor))
static void finalized(void)
{
    record_mask("exit-dtor");
}

static char *probe(const char *name, unsigned int argc, char **argv)
{
    (void)name;
    if (argc != 1)
        _Exit(126);
    if (!strcmp(argv[0], "block-usr2")) {
        sigset_t one;
        if (sigemptyset(&one) < 0 || sigaddset(&one, SIGUSR2) < 0
            || sigprocmask(SIG_BLOCK, &one, NULL) < 0)
            _Exit(126);
    } else if (!strcmp(argv[0], "discard-stdout")) {
        /* Only explicit help/version cases use this to omit branding. */
        int sink = open("/dev/null", O_WRONLY);
        if (sink < 0 || dup2(sink, STDOUT_FILENO) < 0 || close(sink) < 0)
            _Exit(126);
    }
    record_mask(argv[0]);
    char *result = gmk_alloc(1);
    if (!result)
        _Exit(126);
    *result = '\0';
    return result;
}

int mask_gmk_setup(const struct Place *where)
{
    (void)where;
    record_mask("setup");
    gmk_add_function("mask_probe", probe, 1, 1, 0);
    return 1;
}
