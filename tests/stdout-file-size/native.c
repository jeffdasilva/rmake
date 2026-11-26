/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original public stdio and process-finalization witness. */
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <fcntl.h>
#include <libintl.h>
#include <locale.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

int plugin_is_GPL_compatible;

static void mark(const char *path, const char *bytes)
{
    int fd = open(path, O_WRONLY | O_CREAT | O_EXCL, 0644);
    if (fd >= 0) {
        if (write(fd, bytes, 2) != 2)
            (void)unlink(path);
        (void)close(fd);
    }
}

static void before_close(void)
{
    mark("atexit-seen", fcntl(STDOUT_FILENO, F_GETFD) >= 0
        ? (ferror(stdout) ? "E\n" : "O\n") : "C\n");
    if (getenv("PROBE_BUFFERED_AT_EXIT"))
        (void)fputs("ABCDEFGH", stdout);
}

__attribute__((destructor))
static void after_close(void)
{
    mark("fini-seen", fcntl(STDOUT_FILENO, F_GETFD) < 0 ? "C\n" : "O\n");
}

int native_probe_gmk_setup(void *location)
{
    (void)location;
    if (getenv("PROBE_STDOUT_CATALOG") &&
        (!bindtextdomain("make", "./make-catalog") ||
         !setlocale(LC_MESSAGES, "fr_FR.UTF-8")))
        abort();
    if (atexit(before_close))
        return 0;
    if (getenv("NATIVE_WRITE")) {
        (void)fputs("ABCDEFGH\n", stdout);
        (void)fflush(stdout);
    }
    const char *encoding = getenv("PROBE_ERRNO_CODESET");
    if (encoding) {
        if (!bindtextdomain("libc", "./errno-catalog") ||
            !setlocale(LC_MESSAGES, "fr_FR.UTF-8") ||
            !bind_textdomain_codeset("libc", encoding))
            abort();
        /* A filename can preserve the witness under the same write limit. */
        char witness[80];
        int length = snprintf(witness, sizeof(witness), "errno-%s", strerror(EFBIG));
        if (length < 0 || (size_t)length >= sizeof(witness))
            abort();
        mark(witness, "1\n");
    }
    return 1;
}
