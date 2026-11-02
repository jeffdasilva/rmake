/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original bounded Linux/glibc source-reader fault observer. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
static FILE *(*next_open)(const char *,const char *);
static char *(*next_gets)(char *,int,FILE *);
static size_t (*next_read)(void *,size_t,size_t,FILE *);
static int (*next_error)(FILE *);
static void (*next_clear)(FILE *);
static int (*next_close)(FILE *);
static FILE *watched;
static int calls, fired, virtual_error, opened;
static const char *mode;
static void init(void) {
  if (next_open) return;
  next_open=dlsym(RTLD_NEXT,"fopen"); next_gets=dlsym(RTLD_NEXT,"fgets");
  next_read=dlsym(RTLD_NEXT,"fread"); next_error=dlsym(RTLD_NEXT,"ferror");
  next_clear=dlsym(RTLD_NEXT,"clearerr"); next_close=dlsym(RTLD_NEXT,"fclose");
  mode=getenv("RMAKE_SOURCE_FAULT"); if(!mode)mode="normal";
}
static void mark(const char *text) {
  int saved=errno; int fd=open("source.trace",O_WRONLY|O_CREAT|O_APPEND|O_CLOEXEC,0600);
  if(fd>=0){size_t n=strlen(text);ssize_t written=write(fd,text,n);if(written<0){}close(fd);} errno=saved;
}
FILE *fopen(const char *path,const char *flags) {
  init(); const char *target=getenv("RMAKE_SOURCE_TARGET");
  const char *base=strrchr(path,'/');base=base?base+1:path;
  int selected=target && strcmp(target,base)==0 && flags[0]=='r';
  if(selected && !opened && strcmp(mode,"open-eintr")==0){opened=1;mark("open-eintr\n");errno=EINTR;return NULL;}
  FILE *f=next_open(path,flags);
  if(selected && f){watched=f;calls=0;virtual_error=0;opened=1;}
  return f;
}
static int fault(FILE *f,int at) {
  if(f!=watched || fired || ++calls!=at)return 0;
  if(strcmp(mode,"read-eio") && strcmp(mode,"read-eintr") && strcmp(mode,"signal"))return 0;
  fired=1; virtual_error=1;
  if(strcmp(mode,"signal")==0){mark("signal\n");raise(SIGTERM);errno=EINTR;}
  else if(strcmp(mode,"read-eintr")==0){mark("read-eintr\n");errno=EINTR;}
  else{mark("read-eio\n");errno=EIO;}
  return 1;
}
char *fgets(char *s,int n,FILE *f){init();if(fault(f,3))return NULL;return next_gets(s,n,f);}
/* Qualify the preceding whole-source implementation too: its second fread
 * reaches EOF before it has interpreted any line. No Make semantics supplied. */
size_t fread(void *s,size_t a,size_t b,FILE *f){init();if(fault(f,2))return 0;return next_read(s,a,b,f);}
int ferror(FILE *f){init();return f==watched && virtual_error ? 1 : next_error(f);}
void clearerr(FILE *f){init();if(f==watched)virtual_error=0;next_clear(f);}
int fclose(FILE *f){
  init(); int selected=f==watched; int result=next_close(f);
  if(selected){watched=NULL;virtual_error=0;if(!fired && strcmp(mode,"close-eio")==0){fired=1;mark("close-eio\n");errno=EIO;return EOF;}}
  return result;
}
