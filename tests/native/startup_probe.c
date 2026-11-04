/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original startup/finalizer ownership fixture; no GNU headers included. */
#include <stdio.h>
#include <stdio_ext.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
struct Place {const char *file;unsigned long line;};
extern char *gmk_expand(const char *);
extern void gmk_free(char *);
extern void gmk_eval(const char *,const struct Place *);
int plugin_is_GPL_compatible=1;
static char seen_path[8192],module_path[8192];
static void mark(const char *s){FILE*f=fopen(seen_path,"a");if(!f)_Exit(90);fputs(s,f);if(fclose(f))_Exit(91);}
static void finish(void){
    mark("FINAL\n");
    if(getenv("NATIVE_PENDING")){char pending[64];snprintf(pending,sizeof pending,"PENDING=%zu\n",__fpending(stdout));mark(pending);}
    char *user=gmk_expand("$(USER_VALUE)"),*uo=gmk_expand("$(origin USER_VALUE)");
    char *module=gmk_expand("$(MODULE_VALUE)"),*mo=gmk_expand("$(origin MODULE_VALUE)");
    char *pass=gmk_expand("$(PASS)"),*po=gmk_expand("$(origin PASS)");
    char *restart=gmk_expand("$(MAKE_RESTARTS)"),*ro=gmk_expand("$(origin MAKE_RESTARTS)");
    char *loaded=gmk_expand("$(notdir $(.LOADED))"),*lo=gmk_expand("$(origin .LOADED)");
    char *raw=gmk_expand("$(.LOADED)"),*cwd=gmk_expand("$(CURDIR)"),*co=gmk_expand("$(origin CURDIR)");
    char *dirs=gmk_expand("$(filter inc home/inc,$(.INCLUDE_DIRS))"),*io=gmk_expand("$(origin .INCLUDE_DIRS)");
    char physical[8192];if(!getcwd(physical,sizeof physical))_Exit(92);
    const char *base=strrchr(physical,'/');base=base?base+1:physical;
    FILE*f=fopen(seen_path,"a");if(!f)_Exit(90);
    fprintf(f,"STATE USER=[%s]:%s MODULE=[%s]:%s PASS=[%s]:%s RESTART=[%s]:%s LOADED=[%s]:%s MATCH=%d CURDIR=[%s]:%s DIRS=[%s]:%s OS=%s\n",user,uo,module,mo,pass,po,restart,ro,loaded,lo,!strcmp(raw,module_path),!cwd[0]?"empty":!strcmp(cwd,physical)?"matches":"different",co,dirs,io,base);if(fclose(f))_Exit(91);
    char text[2*sizeof seen_path+128];const char *fatal=getenv("FINAL_FATAL");
    if(fatal && !strcmp(fatal,"1")){
        snprintf(text,sizeof text,"$(file >>%s,BEFORE-FATAL)$(error FINAL-STOP)$(file >>%s,FORBIDDEN)\n",seen_path,seen_path);
    }else snprintf(text,sizeof text,"$(file >>%s,FINAL-EVAL)\n",seen_path);
    gmk_eval(text,NULL);
    mark("AFTER\n");
    gmk_free(user);gmk_free(uo);gmk_free(module);gmk_free(mo);gmk_free(pass);gmk_free(po);gmk_free(restart);gmk_free(ro);gmk_free(loaded);gmk_free(lo);gmk_free(raw);gmk_free(cwd);gmk_free(co);gmk_free(dirs);gmk_free(io);
}
int startup_probe_gmk_setup(const struct Place *where){
    (void)where;char cwd[4096];if(!getcwd(cwd,sizeof cwd))return 0;
    snprintf(seen_path,sizeof seen_path,"%s/seen",cwd);
    snprintf(module_path,sizeof module_path,"startup_probe.so");
    if(!freopen("/dev/null","w",stdout))return 0;
    if(getenv("NATIVE_PENDING")){if(setvbuf(stdout,NULL,_IOLBF,0))return 0;fputs("native-partial",stdout);}
    mark("SETUP\n");gmk_eval("MODULE_VALUE := setup\n",NULL);
    if(atexit(finish))return 0;
    return 1;
}
