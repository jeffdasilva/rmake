/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva */
#define _GNU_SOURCE
#include <fcntl.h>
#include <string.h>
#include <unistd.h>
extern char *program_invocation_short_name;
__attribute__((constructor)) static void entry(void) {
 if(!program_invocation_short_name||strcmp(program_invocation_short_name,"make"))return;
 int fd=open("seen",O_WRONLY|O_APPEND|O_CLOEXEC); if(fd<0)_exit(121);
 ssize_t count=write(fd,"ENTRY\n",6);int closed=close(fd);
 if(count!=6||closed)_exit(122);
}
