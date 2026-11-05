// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

#define _GNU_SOURCE
#include <locale.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#ifndef PROFILE_LOCPATH
#error "The test runner must supply its independently verified locale data path"
#endif

static void __attribute__((constructor))
select_messages_before_main(void)
{
    const char *const mode = getenv("RMAKE_TEST_DB_MESSAGES");
    if (mode == NULL)
        _exit(111);
    bool numeric = strcmp(mode, "fr-numeric") == 0;
    bool messages_only = strcmp(mode, "fr") == 0;
    if ((!numeric && !messages_only && strcmp(mode, "C") != 0) ||
        unsetenv("LD_PRELOAD") != 0)
        _exit(111);
    if (numeric || messages_only) {
        if (setenv("LOCPATH", PROFILE_LOCPATH, 1) != 0)
            _exit(112);
        locale_t base = newlocale(LC_ALL_MASK, "C.UTF-8", (locale_t)0);
        if (base == (locale_t)0)
            _exit(113);
        int categories = LC_MESSAGES_MASK;
        if (numeric)
            categories |= LC_NUMERIC_MASK;
        locale_t messages = newlocale(categories, "fr_FR.UTF-8", base);
        if (messages == (locale_t)0 || uselocale(messages) == (locale_t)0 ||
            unsetenv("LOCPATH") != 0)
            _exit(114);
    }
    if (unsetenv("RMAKE_TEST_DB_MESSAGES") != 0)
        _exit(115);
}
