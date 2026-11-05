/* SPDX-License-Identifier: GPL-3.0-or-later
 * Original probe for live libc error messages during remake.
 */
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <libintl.h>
#include <locale.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

typedef char *(*make_callback)(const char *, unsigned int, char **);
extern void gmk_add_function(const char *, make_callback, unsigned int, unsigned int, unsigned int);

static char *remake_locale(const char *name, unsigned int count, char **args) {
    (void)name;
    (void)count;
    (void)args;
    if (access("plain/child", F_OK) != -1)
        abort();
    int unrelated_error = errno;
    if (!bindtextdomain("libc", "./errno-catalog") || !setlocale(LC_MESSAGES, "fr_FR.UTF-8"))
        abort();
    FILE *seen = fopen("seen", "a");
    if (!seen)
        abort();
    fprintf(seen, "original=%s\n", strerror(ENOENT));
    fprintf(seen, "unrelated=%s\n", strerror(unrelated_error));
    if (fclose(seen))
        abort();
    return NULL;
}

int plugin_is_GPL_compatible;

int include_errno_gmk_setup(const void *location) {
    (void)location;
    gmk_add_function("nls_errno_fr", remake_locale, 0, 1, 0);
    return 1;
}
