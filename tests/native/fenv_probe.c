/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original public-ABI fixture; no GNU header, implementation or tests reused. */
#include <fenv.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct Location { const char *file; unsigned long line; };
typedef char *(*Callback)(const char *, unsigned int, char **);
extern char *gmk_alloc(unsigned int);
extern void gmk_free(char *);
extern char *gmk_expand(const char *);
extern void gmk_eval(const char *, const struct Location *);
extern void gmk_add_function(const char *, Callback, unsigned int, unsigned int, unsigned int);
int plugin_is_GPL_compatible = 1;

static const char *round_name(void) {
    switch (fegetround()) {
    case FE_TONEAREST: return "nearest";
    case FE_UPWARD: return "upward";
    case FE_DOWNWARD: return "downward";
    case FE_TOWARDZERO: return "towardzero";
    default: exit(90);
    }
}
static int round_value(const char *name) {
    if(!strcmp(name,"nearest"))return FE_TONEAREST;
    if(!strcmp(name,"upward"))return FE_UPWARD;
    if(!strcmp(name,"downward"))return FE_DOWNWARD;
    if(!strcmp(name,"towardzero"))return FE_TOWARDZERO;
    exit(91);
}
static int exception_value(const char *name) {
    if(!strcmp(name,"divbyzero"))return FE_DIVBYZERO;
    if(!strcmp(name,"overflow"))return FE_OVERFLOW;
    exit(92);
}
static void set_round(int value) { if(fesetround(value))exit(93); }
static void mark(const char *name) {
    FILE *file=fopen("seen","a");
    if(!file)exit(94);
    if(fprintf(file,"%s:%s\n",name,round_name())<0||fclose(file))exit(95);
}
#ifdef FP_FINALIZER
static void value_mark(const char *name,const char *value) {
    FILE *file=fopen("seen","a");
    if(!file)exit(94);
    if(fprintf(file,"%s:%s\n",name,value)<0||fclose(file))exit(95);
}
#endif
static char *copy(const char *value) {
    size_t size=strlen(value)+1;
    char *out=gmk_alloc((unsigned int)size);
    memcpy(out,value,size);return out;
}
static char *query(const char *name,unsigned int argc,char **argv) {
    (void)name;(void)argc;(void)argv;mark("query");return copy(round_name());
}
static char *select_round(const char *name,unsigned int argc,char **argv) {
    (void)name;if(argc!=1)exit(96);
    set_round(round_value(argv[0]));
    char label[80];int size=snprintf(label,sizeof label,"set-%s",argv[0]);
    if(size<0||(size_t)size>=sizeof label)exit(97);
    mark(label);return copy(round_name());
}
static char *nested(const char *name,unsigned int argc,char **argv,int evaluate,int restore) {
    (void)name;
    if(!evaluate&&argc!=1)exit(96);
    fenv_t saved;
    if(restore&&fegetenv(&saved))exit(98);
    set_round(FE_UPWARD);
    mark(evaluate?(restore?"saved-eval-enter":"eval-enter"):(restore?"saved-enter":"expand-enter"));
    const char *before=round_name();
    char *value;
    if(evaluate) {
        gmk_eval("FP_CAPTURE := $(fp_get )\nFP_SIDE := $(fp_set towardzero)\n",NULL);
        value=gmk_expand("$(FP_CAPTURE)");
    } else value=gmk_expand(argv[0]);
    const char *changed=round_name();
    if(restore) {
        mark("saved-before-restore");
        if(fesetenv(&saved))exit(98);
    }
    char result[256];
    int size=restore?snprintf(result,sizeof result,"%s/%s/%s/%s",before,value,changed,round_name())
                    :snprintf(result,sizeof result,"%s/%s/%s",before,value,changed);
    if(size<0||(size_t)size>=sizeof result)exit(97);
    gmk_free(value);
    mark(evaluate?(restore?"saved-eval-return":"eval-return"):(restore?"saved-return":"expand-return"));
    return copy(result);
}
static char *expand(const char *name,unsigned int argc,char **argv) {
    return nested(name,argc,argv,0,0);
}
static char *evaluate(const char *name,unsigned int argc,char **argv) {
    return nested(name,argc,argv,1,0);
}
static char *saved_expand(const char *name,unsigned int argc,char **argv) {
    return nested(name,argc,argv,0,1);
}
static char *saved_eval(const char *name,unsigned int argc,char **argv) {
    return nested(name,argc,argv,1,1);
}
static char *clear_flags(const char *name,unsigned int argc,char **argv) {
    (void)name;(void)argc;(void)argv;
    if(feclearexcept(FE_ALL_EXCEPT))exit(99);
    mark("clear");return copy("cleared");
}
static char *raise_flag(const char *name,unsigned int argc,char **argv) {
    (void)name;if(argc!=1)exit(96);
    if(feraiseexcept(exception_value(argv[0])))exit(99);
    char label[80];int size=snprintf(label,sizeof label,"raise-%s",argv[0]);
    if(size<0||(size_t)size>=sizeof label)exit(97);
    mark(label);return copy("raised");
}
static char *query_flag(const char *name,unsigned int argc,char **argv) {
    (void)name;if(argc!=1)exit(96);
    const char *value=fetestexcept(exception_value(argv[0]))?"1":"0";
    char label[80];int size=snprintf(label,sizeof label,"flag-%s=%s",argv[0],value);
    if(size<0||(size_t)size>=sizeof label)exit(97);
    mark(label);return copy(value);
}
static char *nested_flags(const char *name,unsigned int argc,char **argv) {
    (void)name;(void)argc;(void)argv;
    if(feclearexcept(FE_ALL_EXCEPT)||feraiseexcept(FE_DIVBYZERO))exit(99);
    mark("flags-enter");
    char *inside=gmk_expand("$(fp_flag divbyzero)");
    char *raised=gmk_expand("$(fp_raise overflow)");
    char result[80];int size=snprintf(result,sizeof result,"%s/%d/%d",inside,
        !!fetestexcept(FE_DIVBYZERO),!!fetestexcept(FE_OVERFLOW));
    if(size<0||(size_t)size>=sizeof result)exit(97);
    gmk_free(inside);gmk_free(raised);mark("flags-return");return copy(result);
}
static char *saved_flags(const char *name,unsigned int argc,char **argv) {
    (void)name;(void)argc;(void)argv;
    set_round(FE_TONEAREST);
    if(feclearexcept(FE_ALL_EXCEPT))exit(99);
    fenv_t saved;if(fegetenv(&saved)||feraiseexcept(FE_DIVBYZERO))exit(99);
    mark("saved-flags-enter");
    char *raised=gmk_expand("$(fp_raise overflow)");
    int changed=!!fetestexcept(FE_OVERFLOW);
    if(fesetenv(&saved))exit(99);
    char result[80];int size=snprintf(result,sizeof result,"%d/%d/%d/%s",changed,
        !!fetestexcept(FE_DIVBYZERO),!!fetestexcept(FE_OVERFLOW),round_name());
    if(size<0||(size_t)size>=sizeof result)exit(97);
    gmk_free(raised);mark("saved-flags-return");return copy(result);
}
#ifdef FP_CONSTRUCTOR_MODE
__attribute__((constructor)) static void constructor(void) {
    set_round(FP_CONSTRUCTOR_MODE);mark("constructor");
}
#endif
#ifdef FP_FINALIZER
__attribute__((destructor)) static void finalizer(void) {
    mark("final-enter");
    char *value=gmk_expand("$(fp_get )");
    value_mark("final-value",value);gmk_free(value);mark("final-return");
}
#endif
int fenv_gmk_setup(const struct Location *location) {
    (void)location;
    mark("setup-enter");
#ifdef FP_SETUP_MODE
    set_round(FP_SETUP_MODE);mark("setup-set");
#endif
    gmk_add_function("fp_get",query,0,0,0);
    gmk_add_function("fp_set",select_round,1,1,0);
    gmk_add_function("fp_expand",expand,1,1,1);
    gmk_add_function("fp_eval",evaluate,0,0,0);
    gmk_add_function("fp_saved",saved_expand,1,1,1);
    gmk_add_function("fp_saved_eval",saved_eval,0,0,0);
    gmk_add_function("fp_clear",clear_flags,0,0,0);
    gmk_add_function("fp_raise",raise_flag,1,1,0);
    gmk_add_function("fp_flag",query_flag,1,1,0);
    gmk_add_function("fp_flags_nested",nested_flags,0,0,0);
    gmk_add_function("fp_flags_saved",saved_flags,0,0,0);
    mark("setup-exit");return 1;
}
