/* SPDX-License-Identifier: GPL-3.0-or-later */
#include <stdio.h>
#include <stdlib.h>
int plugin_is_GPL_compatible=1;
static void mark(const char*s){FILE*f=fopen("seen","a");if(!f)_Exit(90);fputs(s,f);if(fclose(f))_Exit(91);}
__attribute__((constructor))static void begin(void){mark("CTOR\n");}
__attribute__((destructor))static void end(void){mark("FINI\n");}
int probe_gmk_setup(const void*p){(void)p;mark("SETUP\n");return 1;}
