/* SPDX-License-Identifier: GPL-3.0-or-later
 * Original LC_MESSAGES phase witness; public libc and module ABI only. */
#define _GNU_SOURCE
#include <locale.h>
#include <stdlib.h>
typedef char *(*function)(const char *,unsigned int,char **);
extern void gmk_add_function(const char *,function,unsigned int,unsigned int,unsigned int);
static char *select_c(const char *name,unsigned int argc,char **argv){
 (void)name;(void)argc;(void)argv;
 if(!setlocale(LC_MESSAGES,"C")){abort();}
 return NULL;
}
static char *select_fr(const char *name,unsigned int argc,char **argv){
 (void)name;(void)argc;(void)argv;
 if(!setlocale(LC_MESSAGES,"fr_FR.UTF-8")){abort();}
 return NULL;
}
int plugin_is_GPL_compatible;
int nls_probe_gmk_setup(const void *location){
 (void)location;
 gmk_add_function("nls_c",select_c,0,1,0);
 gmk_add_function("nls_fr",select_fr,0,1,0);
 return 1;
}
