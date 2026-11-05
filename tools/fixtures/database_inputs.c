/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original public-libc input/format witness; no GNU implementation or headers. */
#define _GNU_SOURCE
#include <dirent.h>
#include <errno.h>
#include <fenv.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

static int number(const char *text, uint64_t *value) {
  if (!text[0] || text[0]=='-') return 0;
  errno=0; char *end; unsigned long long parsed=strtoull(text,&end,10);
  if (errno || *end || parsed>1000000000ULL) return 0;
  *value=(uint64_t)parsed; return 1;
}
static int numeric(int argc,char **argv) {
  uint64_t numerator,denominator;
  if(argc!=6 || !number(argv[2],&numerator) || !number(argv[3],&denominator)
      || !denominator) return 90;
  int mode;
  if(!strcmp(argv[4],"nearest")) mode=FE_TONEAREST;
  else if(!strcmp(argv[4],"downward")) mode=FE_DOWNWARD;
  else if(!strcmp(argv[4],"upward")) mode=FE_UPWARD;
  else if(!strcmp(argv[4],"towardzero")) mode=FE_TOWARDZERO;
  else return 91;
  if(fesetround(mode) || fegetround()!=mode) return 92;
  volatile double count=(double)numerator, total=(double)denominator;
  volatile double scaled=100.0*count;
  volatile double ratio=scaled/total;
  if(!strcmp(argv[5],"rounded")) printf("%.0f\n",ratio);
  else if(!strcmp(argv[5],"truncated")) printf("%lu\n",(unsigned long)ratio);
  else return 93;
  return ferror(stdout) ? 94 : 0;
}
static int directories(int argc,char **argv) {
  size_t total=0,bytes=0;
  for(int index=2;index<argc;++index) {
    DIR *directory=opendir(argv[index]);
    if(!directory) { printf("D\t%d\t%d\n",index-2,errno);continue; }
    struct stat metadata;
    if(fstat(dirfd(directory),&metadata)) { closedir(directory);return 95; }
    printf("D\t%d\t0\t%ju\t%ju\n",index-2,(uintmax_t)metadata.st_dev,(uintmax_t)metadata.st_ino);
    for(;;) {
      errno=0;struct dirent *entry=readdir(directory);
      if(!entry) { if(errno) { closedir(directory);return 96; }break; }
      size_t length=strlen(entry->d_name);++total;bytes+=length;
      if(total>20000 || bytes>1048576) { closedir(directory);return 97; }
      printf("E\t%d\t%ju\t%u\t",index-2,(uintmax_t)entry->d_ino,(unsigned)entry->d_type);
      for(size_t offset=0;offset<length;++offset) printf("%02x",(unsigned char)entry->d_name[offset]);
      putchar('\n');
    }
    if(closedir(directory)) return 98;
    printf("Z\t%d\n",index-2);
  }
  return ferror(stdout) ? 99 : 0;
}
int main(int argc,char **argv) {
  if(argc>=2 && !strcmp(argv[1],"numeric")) return numeric(argc,argv);
  if(argc>=2 && !strcmp(argv[1],"directories")) return directories(argc,argv);
  return 89;
}
