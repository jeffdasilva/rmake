/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original resource and recursion witness. */
#define _POSIX_C_SOURCE 200809L
#include <fcntl.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

static int mark(const char *name)
{
    int fd = open(name, O_CREAT | O_EXCL | O_WRONLY, 0644);
    return fd >= 0 && close(fd) == 0;
}

static int await(const char *name)
{
    const struct timespec interval = {0, 1000000};
    for (int tries = 0; tries < 3400; ++tries) {
        if (access(name, F_OK) == 0)
            return 1;
        nanosleep(&interval, NULL);
    }
    return 0;
}

static int private_fds(int first)
{
    struct rlimit limit;
    struct stat st;
    if (getrlimit(RLIMIT_NOFILE, &limit) != 0 || limit.rlim_cur > 64)
        return 0;
    for (int fd = first; fd < (int)limit.rlim_cur; ++fd)
        if (fstat(fd, &st) == 0)
            return 0;
    return 1;
}

static int inherited_descriptor(void)
{
    struct stat inherited, named;
    char content[16];
    if (!private_fds(4) || fstat(3, &inherited) || stat("inherited.txt", &named)
        || !S_ISREG(inherited.st_mode) || inherited.st_dev != named.st_dev
        || inherited.st_ino != named.st_ino || fcntl(3, F_GETFD) != 0
        || pread(3, content, sizeof(content), 0) != 16
        || memcmp(content, "inherited input\n", sizeof(content)) || lseek(3, 0, SEEK_CUR) != 0)
        return 102;
    return dprintf(1, "inherited=3 same-file open offset=0\n") < 0;
}

static int recursion(int outer)
{
    const char key[] = "--jobserver-auth=fifo:";
    const char *flags = getenv("MAKEFLAGS");
    const char *directory = getenv("TMPDIR");
    const char *level = getenv("MAKELEVEL");
    if (!flags || !directory || !level || strcmp(level, outer ? "1" : "2"))
        return 91;
    const char *path_start = strstr(flags, key);
    if (!path_start)
        return 92;
    path_start += sizeof(key) - 1;
    size_t length = strcspn(path_start, " \t");
    char path[4096];
    if (!length || length >= sizeof(path))
        return 93;
    memcpy(path, path_start, length);
    path[length] = 0;
    size_t dir_length = strlen(directory);
    if (strncmp(path, directory, dir_length) || strncmp(path + dir_length, "/GMfifo", 7))
        return 94;
    if (outer) {
        char name[48];
        snprintf(name, sizeof(name), "/GMfifo%jd", (intmax_t)getppid());
        if (strcmp(path + dir_length, name))
            return 95;
    }
    int fd = open(path, O_RDONLY | O_NONBLOCK | O_CLOEXEC);
    int available = -1;
    struct stat st;
    if (fd < 0 || fstat(fd, &st) || !S_ISFIFO(st.st_mode) || (st.st_mode & 0777) != 0600
        || ioctl(fd, FIONREAD, &available) || available != 1 || close(fd))
        return 96;
    FILE *identity = fopen(".pool-identity", outer ? "wx" : "r");
    if (!identity)
        return 97;
    if (outer) {
        if (fprintf(identity, "%ju %ju\n", (uintmax_t)st.st_dev, (uintmax_t)st.st_ino) < 0
            || fclose(identity))
            return 98;
    } else {
        uintmax_t device, inode;
        if (fscanf(identity, "%ju %ju", &device, &inode) != 2 || fclose(identity)
            || device != (uintmax_t)st.st_dev || inode != (uintmax_t)st.st_ino
            || unlink(".pool-identity"))
            return 99;
    }
    return printf("%s level=%s fifo=0600 tokens=1 inherited=0\n",
                  outer ? "outer" : "inner", level) < 0;
}

int main(int argc, char **argv)
{
    if (argc != 2)
        return 90;
    const char *mode = argv[1];
    if (!strcmp(mode, "descriptor"))
        return inherited_descriptor();
    if (!private_fds(3))
        return 90;
    if (!strcmp(mode, "outer") || !strcmp(mode, "inner"))
        return recursion(!strcmp(mode, "outer"));
    if (!strcmp(mode, "a")) {
        if (dprintf(1, "a:start\n") < 0 || !mark("a.ready")
            || !await("a.release") || dprintf(1, "a:done\n") < 0)
            return 100;
        return 0;
    }
    if (strcmp(mode, "b") || !await("a.ready") || dprintf(1, "b:start\n") < 0
        || !mark("b.held") || !await("b.release") || dprintf(1, "b:done\n") < 0)
        return 101;
    return 0;
}
