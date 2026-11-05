/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original jobserver wire and controlled recipe witness. */
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <fcntl.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

static int mark(const char *path)
{
    int fd = open(path, O_WRONLY | O_CREAT | O_EXCL, 0644);
    return fd >= 0 && close(fd) == 0;
}

static int wait_marker(const char *path)
{
    const struct timespec delay = {0, 1000000};
    for (int count = 0; count < 3800; ++count) {
        if (access(path, F_OK) == 0)
            return 1;
        nanosleep(&delay, NULL);
    }
    return 0;
}

static int inspect(const char *kind)
{
    if (!strcmp(kind, "ordinary")) {
        int read_closed = fcntl(3, F_GETFD) < 0 && errno == EBADF;
        int write_closed = fcntl(4, F_GETFD) < 0 && errno == EBADF;
        if (!read_closed || !write_closed)
            return 91;
        puts("ordinary|wire-private");
        return 0;
    }
    const char *flags = getenv("MAKEFLAGS");
    const char *auth = flags ? strstr(flags, "--jobserver-auth=") : NULL;
    const char *device = getenv("RM_PIPE_DEV");
    const char *inode = getenv("RM_PIPE_INO");
    struct stat read_end, write_end;
    const char declared[] = "--jobserver-auth=3,4";
    if (!auth || strncmp(auth, declared, sizeof(declared) - 1)
        || (auth[sizeof(declared) - 1] != '\0' && auth[sizeof(declared) - 1] != ' ')
        || !device || !inode
        || fstat(3, &read_end) < 0 || fstat(4, &write_end) < 0
        || !S_ISFIFO(read_end.st_mode) || !S_ISFIFO(write_end.st_mode)
        || read_end.st_dev != write_end.st_dev || read_end.st_ino != write_end.st_ino
        || (uintmax_t)read_end.st_dev != strtoumax(device, NULL, 10)
        || (uintmax_t)read_end.st_ino != strtoumax(inode, NULL, 10)
        || fcntl(3, F_GETFD) != 0 || fcntl(4, F_GETFD) != 0)
        return 92;
    int read_flags = fcntl(3, F_GETFL), write_flags = fcntl(4, F_GETFL);
    if (read_flags < 0 || write_flags < 0 || !(read_flags & O_NONBLOCK)
        || (write_flags & O_NONBLOCK) || (read_flags & O_ACCMODE) != O_RDONLY
        || (write_flags & O_ACCMODE) != O_WRONLY)
        return 93;
    puts("recursive|wire=3,4|shared-pipe|read=nonblocking|write=blocking");
    return 0;
}

static int worker(char name, int status)
{
    if (name == 'b' && !wait_marker("a.ready"))
        return 94;
    char ready[] = "a.ready", release[] = "a.release", done[] = "a.done";
    ready[0] = release[0] = done[0] = name;
    struct stat input;
    if (fstat(STDIN_FILENO, &input) < 0)
        return 95;
    if (dprintf(STDOUT_FILENO, "%c:start:%s\n", name,
                S_ISFIFO(input.st_mode) ? "fifo" : "original") < 0
        || !mark(ready) || !wait_marker(release)
        || dprintf(STDOUT_FILENO, "%c:done\n", name) < 0 || !mark(done))
        return 96;
    return status;
}

int main(int argc, char **argv)
{
    if (argc == 2)
        return inspect(argv[1]);
    if (argc == 3 && (argv[1][0] == 'a' || argv[1][0] == 'b'))
        return worker(argv[1][0], atoi(argv[2]));
    return 97;
}
