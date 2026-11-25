/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva */
#define _POSIX_C_SOURCE 200809L
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

struct Source { const char *filename; unsigned long line; };
extern void gmk_eval(const char *, const struct Source *);
extern char *gmk_expand(const char *);
extern void gmk_free(char *);
int plugin_is_GPL_compatible = 1;

static void record(const char *name, const char *value) {
    FILE *file = fopen("seen", "a");
    if (!file) return;
    fprintf(file, "%s=[%s]\n", name, value);
    fclose(file);
}

static void record_stdout(const char *name) {
    record(name, fcntl(STDOUT_FILENO, F_GETFD) < 0 ? "closed" : "open");
}

static void capture(void) {
    const char *form = getenv("PROBE_FINALIZER_SHELL_FORM");
    record_stdout("stdout-before");
    if (getenv("PROBE_FINALIZER_REOPEN_STDOUT")) {
        int fd = open("/dev/null", O_WRONLY);
        if (fd < 0) return;
        if (fd != STDOUT_FILENO) {
            int installed = dup2(fd, STDOUT_FILENO);
            close(fd);
            if (installed < 0) return;
        }
        record_stdout("stdout-reopened");
    }
    if (form && !strcmp(form, "assignment")) {
        gmk_eval("CAPTURED != printf 'spawned\\n' >>seen; printf captured; exit 13", NULL);
    } else if (form && !strcmp(form, "missing")) {
        gmk_eval("CAPTURED := $(shell rmake-native-finalizer-missing-command)", NULL);
    } else {
        gmk_eval("CAPTURED := $(shell printf 'spawned\\n' >>seen; printf captured; exit 13)", NULL);
    }
    char *result = gmk_expand("$(CAPTURED)|$(.SHELLSTATUS)|$(origin .SHELLSTATUS)");
    record("result", result);
    gmk_free(result);
    record_stdout("stdout-after");
}

__attribute__((destructor)) static void finalize(void) {
    if (!getenv("PROBE_FINALIZER_ATEXIT")) capture();
}

int finalizer_shell_gmk_setup(const struct Source *location) {
    (void)location;
    return !getenv("PROBE_FINALIZER_ATEXIT") || !atexit(capture);
}
