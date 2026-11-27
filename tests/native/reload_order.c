/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Record the lifetime of each unchanged native module. */
#include <stdio.h>

#ifdef MODULE_B
#define LABEL "b"
#define SETUP module_b_gmk_setup
#else
#define LABEL "a"
#define SETUP module_a_gmk_setup
#endif

int plugin_is_GPL_compatible;

static void append(const char *event)
{
    FILE *out = fopen("seen", "a");
    if (out != NULL) {
        fprintf(out, "%s:%s\n", LABEL, event);
        fclose(out);
    }
}

__attribute__((constructor)) static void constructor(void)
{
    append("open");
}

__attribute__((destructor)) static void destructor(void)
{
    append("close");
}

int SETUP(const void *location)
{
    (void)location;
    append("setup");
    return 1;
}
