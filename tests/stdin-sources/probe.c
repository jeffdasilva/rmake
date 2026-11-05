/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original public ABI fixture; no GNU header, implementation or tests. */
#define _GNU_SOURCE
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
struct Place { const char *file; unsigned long line; };
extern char *gmk_expand(const char *);
extern void gmk_eval(const char *, const struct Place *);
extern void gmk_free(char *);
extern void gmk_add_function(const char *, char *(*)(const char *,unsigned int,char **), unsigned int,unsigned int,unsigned int);
int plugin_is_GPL_compatible=1;
static char *saved_source;
static void mark(const char *value) {
    const char *path=getenv("SEEN_PATH");
    FILE *file=fopen(path?path:"seen","a");
    if(!file)_Exit(90);
    if(fputs(value,file)<0||fclose(file))_Exit(91);
}
__attribute__((constructor))static void begin(void) {
    mark("CTOR\n");
    if(getenv("SET_NATIVE_VALUE")) {
        const char *value=getenv("NATIVE_VALUE");
        mark(value&&!strcmp(value,"live")?"ENV=live\n":"ENV=seed\n");
        if(setenv("NATIVE_VALUE","live",1))_Exit(93);
    }
    const char *changed=getenv("SET_TMPDIR_TO");
    if(changed){if(setenv("TMPDIR",changed,1))_Exit(92);mark("TMPDIR=changed\n");}
    const char *mode=getenv("STDIN_MODE");
    if(mode&&!strcmp(mode,"peek")) {
        int byte=fgetc(stdin);
        if(byte=='.'&&ungetc(byte,stdin)==byte)mark("PREFETCH=restored\n");
        else mark("PREFETCH=bad\n");
    } else if(mode&&!strcmp(mode,"consume")) {
        if(fgetc(stdin)=='!')mark("CONSUMED=bang\n");else mark("CONSUMED=bad\n");
    }
}
static int same_input(const char *path) {
    const char *input=getenv("SOURCE_INPUT");if(!input)return 0;
    FILE *actual=fopen(path,"rb"),*expected=fopen(input,"rb");
    if(!actual||!expected){if(actual)fclose(actual);if(expected)fclose(expected);return 0;}
    const char *mode=getenv("STDIN_MODE");if(mode&&!strcmp(mode,"consume"))fgetc(expected);
    int ok=1,a,b;size_t size=0;
    do {a=fgetc(actual);b=fgetc(expected);if(a!=b||++size>8192){ok=0;break;}}while(a!=EOF);
    if(ferror(actual)||ferror(expected))ok=0;
    fclose(actual);fclose(expected);return ok;
}
static char *check(const char *name,unsigned int count,char **args) {
    (void)name;(void)count;(void)args;
    char *path=gmk_expand("$(firstword $(MAKEFILE_LIST))");
    const char *manual=getenv("EXPECTED_MANUAL");
    const char *directory=getenv("EXPECTED_DIRECTORY");
    struct stat info;int kind=0;
    if(manual)kind=!strcmp(path,manual);
    else if(directory) {
        size_t prefix=strlen(directory),length=strlen(path);
        kind=length==prefix+9&&!memcmp(path,directory,prefix)&&path[prefix]=='/'&&!memcmp(path+prefix+1,"Gm",2);
    }
    int file_ok=!stat(path,&info)&&S_ISREG(info.st_mode)&&((info.st_mode&0777)==(manual?0644:0600));
    int ok=kind&&file_ok&&same_input(path);
    free(saved_source);saved_source=NULL;
    if(path[0]=='/')saved_source=strdup(path);
    else {char *cwd=getcwd(NULL,0);if(cwd){if(asprintf(&saved_source,"%s/%s",cwd,path)<0)saved_source=NULL;free(cwd);}}
    mark(ok?"CHECK=ok\n":"CHECK=bad\n");gmk_free(path);
    return strdup(ok?"ok":"bad");
}
static char *absent(const char *name,unsigned int count,char **args) {
    (void)name;(void)count;(void)args;
    struct stat info;
    if(saved_source&&stat(saved_source,&info)<0&&errno==ENOENT)mark("UNLINKED=ok\n");
    else mark("UNLINKED=bad\n");
    return strdup("");
}
static char *stop(const char *name,unsigned int count,char **args) {
    (void)name;(void)count;(void)args;
    const struct Place location={"stdin-stop.mk",23};
    gmk_eval("$(error STOP)", &location);
    mark("FORBIDDEN-AFTER-ERROR\n");return strdup("bad");
}
__attribute__((destructor))static void end(void) {
    if(!saved_source){mark("FINI=unobserved\n");return;}
    struct stat info;
    if(stat(saved_source,&info)<0&&errno==ENOENT)mark("FINI=removed\n");
    else mark("FINI=present\n");
}
int stdin_probe_gmk_setup(const struct Place *place) {
    (void)place;mark("SETUP\n");
    gmk_add_function("stdin_check",check,0,0,0);
    gmk_add_function("stdin_stop",stop,0,0,0);
    gmk_add_function("stdin_absent",absent,0,0,0);
    return 1;
}
