/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original directory-output/finalizer fixture; no GNU headers included. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
struct Place {const char *file;unsigned long line;};
extern void gmk_eval(const char *,const struct Place *);
int plugin_is_GPL_compatible=1;
static void mark(const char *text){FILE*f=fopen("seen","a");if(!f)_Exit(90);fputs(text,f);if(fclose(f))_Exit(91);}
static void finalizer(void){
    mark("FINAL\n");
    if(getenv("FINAL_INCLUDE"))gmk_eval("include final.mk\n",NULL);
    else gmk_eval("$(warning FINAL-WARN)\n",NULL);
    mark("AFTER\n");
}
int directory_probe_gmk_setup(const struct Place *where){
    (void)where;
    if(atexit(finalizer))return 0;
    mark("SETUP\n");
    fputs("native-partial",stdout);
    gmk_eval("$(warning SETUP-WARN)\n",NULL);
    return 1;
}
