/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original nullable source provenance fixture; no GNU headers included. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
struct Place { const char *file; unsigned long line; };
extern void gmk_eval(const char *, const struct Place *);
int plugin_is_GPL_compatible=1;
static const char *mode(void){const char *m=getenv("PROV_MODE");return m?m:"floc";}
static struct Place origin(void){
    const char *kind=getenv("PROV_KIND"),*line=getenv("PROV_LINE");
    struct Place p={NULL,line?strtoul(line,NULL,10):1};
    if(kind && !strcmp(kind,"empty"))p.file="";
    if(kind && !strcmp(kind,"named"))p.file="origin.mk";
    if(kind && !strcmp(kind,"builtin"))p.file="<builtin>";
    return p;
}
static void evaluate(const char *text){
    struct Place p=origin();const char *kind=getenv("PROV_KIND");
    gmk_eval(text,kind && !strcmp(kind,"implicit")?NULL:&p);
}
static void mark(const char *text){FILE *f=fopen("seen","a");if(!f)exit(90);fputs(text,f);if(fclose(f))exit(91);}
int provenance_driver_gmk_setup(const struct Place *where){
    (void)where;
    if(!strcmp(mode(),"warning"))evaluate("$(file >>seen,EVAL)\n$(warning eval-warn)\n");
    if(!strcmp(mode(),"recipe"))evaluate("late:\n\t@$(file >>seen,RECIPE)$(warning recipe-warn)printf 'RUN\\n'\n");
    if(!strcmp(mode(),"include"))evaluate("$(file >>seen,BEFORE)\ninclude child.mk\n$(warning restored-warn)\n$(file >>seen,AFTER)\n");
    if(!strcmp(mode(),"reload"))evaluate("load prov_reload.so\n");
    if(!strcmp(mode(),"suffix"))evaluate(".c.o:\n\t@$(file >>seen,SUFFIX)printf 'SUFFIX\\n'\n");
    return 1;
}
__attribute__((destructor))static void finish(void){
    if(strcmp(mode(),"floc"))return;
    mark("DTOR\n");evaluate("load prov_probe.so\n");mark("AFTER\n");
}
