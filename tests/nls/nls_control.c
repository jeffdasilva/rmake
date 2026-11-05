/* SPDX-License-Identifier: GPL-3.0-or-later
 * Original LC_MESSAGES/domain/terminating-callback fixture; public APIs only. */
#define _GNU_SOURCE
#include <locale.h>
#include <libintl.h>
#include <stdio.h>
#include <stdlib.h>
typedef char *(*function)(const char *,unsigned int,char **);
extern void gmk_add_function(const char *,function,unsigned int,unsigned int,unsigned int);
extern void gmk_eval(const char *,const void *);
static void mark(const char *value){FILE *file=fopen("seen","a");if(!file){abort();}fputs(value,file);if(fclose(file)){abort();}}
static void private_selection(const char *name){locale_t base=duplocale(LC_GLOBAL_LOCALE);if(!base){abort();}locale_t selected=newlocale(LC_MESSAGES_MASK,name,base);if(!selected||!uselocale(selected)){abort();}}
static char *global_c(const char *name,unsigned int argc,char **argv){(void)name;(void)argc;(void)argv;if(!setlocale(LC_ALL,"C.UTF-8")||!uselocale(LC_GLOBAL_LOCALE)){abort();}return NULL;}
static char *private_fr(const char *name,unsigned int argc,char **argv){(void)name;(void)argc;(void)argv;private_selection("fr_FR.UTF-8");return NULL;}
static char *private_c(const char *name,unsigned int argc,char **argv){(void)name;(void)argc;(void)argv;private_selection("C");return NULL;}
static char *global_fr(const char *name,unsigned int argc,char **argv){(void)name;(void)argc;(void)argv;if(!setlocale(LC_MESSAGES,"fr_FR.UTF-8")){abort();}return NULL;}
static char *use_global(const char *name,unsigned int argc,char **argv){(void)name;(void)argc;(void)argv;if(!uselocale(LC_GLOBAL_LOCALE)){abort();}return NULL;}
static char *custom(const char *name,unsigned int argc,char **argv){(void)name;(void)argc;(void)argv;if(!bindtextdomain("rmake-nls-fixture","./custom")||!textdomain("rmake-nls-fixture")){abort();}return NULL;}
static char *restore(const char *name,unsigned int argc,char **argv){(void)name;(void)argc;(void)argv;if(!textdomain("make")){abort();}return NULL;}
static char *unbound(const char *name,unsigned int argc,char **argv){(void)name;(void)argc;(void)argv;if(!textdomain("rmake-nls-qualified-unbound-20261104")){abort();}return NULL;}
static char *fatal_private(const char *name,unsigned int argc,char **argv){(void)name;(void)argc;(void)argv;private_selection("fr_FR.UTF-8");mark("before-fatal\n");gmk_eval("$(intcmp bad,0)",NULL);mark("forbidden-after-fatal\n");return NULL;}
#ifdef NLS_FINAL
__attribute__((destructor))static void finish(void){mark("final\n");fprintf(stderr,"FINAL=%s\n",gettext("missing separator"));}
#endif
int plugin_is_GPL_compatible;
int nls_control_gmk_setup(const void *location){
 (void)location;
 gmk_add_function("nls_global_c",global_c,0,1,0);
 gmk_add_function("nls_private_fr",private_fr,0,1,0);
 gmk_add_function("nls_private_c",private_c,0,1,0);
 gmk_add_function("nls_global_fr",global_fr,0,1,0);
 gmk_add_function("nls_use_global",use_global,0,1,0);
 gmk_add_function("nls_custom",custom,0,1,0);
 gmk_add_function("nls_restore",restore,0,1,0);
 gmk_add_function("nls_unbound",unbound,0,1,0);
 gmk_add_function("nls_fatal_private",fatal_private,0,1,0);
 return 1;
}
