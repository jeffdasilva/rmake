// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva
#define _GNU_SOURCE
#include <sys/stat.h>
#include <dlfcn.h>
#include <unistd.h>
#include <stdio.h>
#include <string.h>
/* Original fixture adapter: only archive.a's observed modification time is
   fixed. Real file operations, header writes and other metadata remain live. */
static int archive_path(const char *path) {
 size_t n=path?strlen(path):0;
 return n>=9 && !strcmp(path+n-9,"archive.a") && (n==9 || path[n-10]=='/');
}
static int archive_fd(int fd) {
 char link[64],path[4096];snprintf(link,sizeof link,"/proc/self/fd/%d",fd);
 ssize_t n=readlink(link,path,sizeof path-1);if(n<0)return 0;path[n]=0;
 return archive_path(path);
}
#define FIX(result,selected,st) do { if((result)==0 && (selected)) { (st)->st_mtim.tv_sec=946684800;(st)->st_mtim.tv_nsec=0; } } while(0)
int stat(const char *p,struct stat *s) { int (*real)(const char*,struct stat*)=dlsym(RTLD_NEXT,"stat");int r=real(p,s);FIX(r,archive_path(p),s);return r; }
int stat64(const char *p,struct stat64 *s) { int (*real)(const char*,struct stat64*)=dlsym(RTLD_NEXT,"stat64");int r=real(p,s);FIX(r,archive_path(p),s);return r; }
int fstat(int fd,struct stat *s) { int (*real)(int,struct stat*)=dlsym(RTLD_NEXT,"fstat");int r=real(fd,s);FIX(r,archive_fd(fd),s);return r; }
int fstat64(int fd,struct stat64 *s) { int (*real)(int,struct stat64*)=dlsym(RTLD_NEXT,"fstat64");int r=real(fd,s);FIX(r,archive_fd(fd),s);return r; }
int statx(int fd,const char *p,int flags,unsigned mask,struct statx *s) {
 int (*real)(int,const char*,int,unsigned,struct statx*)=dlsym(RTLD_NEXT,"statx");
 int r=real(fd,p,flags,mask,s);
 if(!r && (archive_path(p) || (!*p && archive_fd(fd)))) {s->stx_mtime.tv_sec=946684800;s->stx_mtime.tv_nsec=0;}
 return r;
}
