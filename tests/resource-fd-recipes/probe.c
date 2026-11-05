/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original process entry witness for recipe launch under descriptor pressure. */
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

extern char **environ;

int main(int argc, char **argv)
{
    if (argc == 2 && !strcmp(argv[1], "exit127")) {
        puts("entered");
        return 127;
    }
    struct sigaction pipe_action;
    sigset_t mask;
    if (argc != 2 || sigaction(SIGPIPE, NULL, &pipe_action) < 0
        || sigprocmask(SIG_SETMASK, NULL, &mask) < 0)
        return 126;
    int fd3_closed = fcntl(3, F_GETFD) < 0 && errno == EBADF;
    printf("%s|%s|%s|%d|%d\n", argv[0], argv[1],
           pipe_action.sa_handler == SIG_IGN ? "ignored" :
           pipe_action.sa_handler == SIG_DFL ? "default" : "caught",
           sigismember(&mask, SIGUSR2), fd3_closed);
    for (char **entry = environ; *entry; ++entry)
        if (!strncmp(*entry, "RM_IMAGE_", 9) || !strncmp(*entry, "MAKELEVEL=", 10))
            puts(*entry);
    return 0;
}
