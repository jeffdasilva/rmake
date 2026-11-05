/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva */
#include "mask_bits.h"
#include <unistd.h>

__attribute__((constructor))
static void entry(void)
{
    const char *selected = getenv("RM_MASK_PID");
    if (selected && strtol(selected, NULL, 10) == (long)getpid())
        record_mask("entry");
}
