/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original primary-setup failure/retry fixture; no GNU headers are included. */
#include <stdio.h>
#include <stdlib.h>
struct Source { const char *filename; unsigned long line; };
extern void gmk_eval(const char *, const struct Source *);
int plugin_is_GPL_compatible = 1;
static unsigned int calls;
static void mark(const char *text) {
    FILE *stream = fopen("seen", "a");
    if (!stream) exit(90);
    fputs(text, stream); fputc('\n', stream);
    if (fclose(stream)) exit(91);
}
int retry_module_gmk_setup(const struct Source *location) {
    (void)location;
    char text[32];
    ++calls;
    snprintf(text, sizeof text, "setup-%u", calls);
    mark(text);
    return calls == 1 ? 0 : 1;
}
__attribute__((destructor)) static void finalize(void) {
    mark("final-enter");
    gmk_eval("load ./retry_module.so", NULL);
    mark("final-after");
}
