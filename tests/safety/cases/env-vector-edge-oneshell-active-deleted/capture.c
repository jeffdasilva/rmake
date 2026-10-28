// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva
#include <stdio.h>
#include <string.h>
extern char **environ;
int main(int argc,char **argv) {
 for(char **entry=environ;*entry;entry++) {
  size_t n=strcspn(*entry,"=");
  if(argc>1 && !strcmp(argv[1],"names")) { fwrite(*entry,1,n,stdout);putchar('\n'); }
  else for(int i=1;i<argc;i++)if(strlen(argv[i])==n&&!memcmp(*entry,argv[i],n)) {puts(*entry);break;}
 }
 return 0;
}
