/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original buffered input boundary; public libc interfaces only. */
#define _GNU_SOURCE
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/stat.h>
#include <unistd.h>
#include <errno.h>

int rm_stdin_create(char *name) {
    mode_t previous=umask(0077);
    int descriptor=mkostemp(name,O_CLOEXEC);
    int error=errno;
    umask(previous);
    errno=error;
    return descriptor;
}
void *rm_stdin_buffer(int descriptor) { return fdopen(descriptor,"w+b"); }
size_t rm_stdin_read(unsigned char *bytes,size_t capacity,int *finished) {
    size_t length=fread(bytes,1,capacity,stdin);
    *finished=feof(stdin)||ferror(stdin);
    return length;
}
int rm_stdin_write(void *output,const unsigned char *bytes,size_t length) {
    return fwrite(bytes,1,length,output)==length?0:-1;
}
void rm_stdin_close(void *output) { (void)fclose(output); }
