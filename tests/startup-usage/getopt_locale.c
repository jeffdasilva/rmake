/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 *
 * Original fixture selecting a private message locale before program entry.
 */
#define _GNU_SOURCE
#include <libintl.h>
#include <locale.h>
#include <stdio.h>
#include <unistd.h>

__attribute__((constructor))
static void select_messages(void) {
    if (!bindtextdomain("libc", "./libc-catalog"))
        _exit(93);
#if PRIVATE_MESSAGES
    locale_t base = newlocale(LC_ALL_MASK, "C.UTF-8", (locale_t)0);
    if (!base)
        _exit(93);
    locale_t selected = newlocale(LC_MESSAGES_MASK, "fr_FR.UTF-8", base);
    if (!selected || !uselocale(selected))
        _exit(93);
#endif
    FILE *seen = fopen("seen", "a");
    if (!seen)
        _exit(93);
    if (fputs("constructor\n", seen) == EOF || fclose(seen))
        _exit(93);
}
