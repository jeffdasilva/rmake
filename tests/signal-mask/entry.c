/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva */
#include "mask_bits.h"

int main(int argc, char **argv)
{
    if (argc != 2)
        return 126;
    observe_mask(stdout, argv[1]);
    return fflush(stdout) == 0 ? 0 : 126;
}
