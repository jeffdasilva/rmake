/* SPDX-License-Identifier: GPL-3.0-or-later
 * Original public-ABI/Guile root fixture, no GNU header or code reused. */
#define _GNU_SOURCE
#define GC_NO_THREAD_REDIRECTS
#include <libguile.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
struct Location { const char *file; unsigned long line; };
typedef char *(*Callback)(const char *, unsigned int, char **);
extern char *gmk_alloc(unsigned int);
extern char *gmk_expand(const char *);
extern void gmk_free(char *);
extern void gmk_add_function(const char *,Callback,unsigned int,unsigned int,unsigned int);
int plugin_is_GPL_compatible=1;
static int live;
#ifdef WORKER_GUILE_FIRST
static pthread_mutex_t initialized_lock=PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t initialized_wake=PTHREAD_COND_INITIALIZER;
static int initialized;
static void *hold_worker(void *unused) {
    (void)unused;pthread_mutex_lock(&initialized_lock);
    initialized=1;pthread_cond_broadcast(&initialized_wake);
    for(;;)pthread_cond_wait(&initialized_wake,&initialized_lock);
    return NULL;
}
static void *worker_inside(void *unused) {
    (void)unused;scm_without_guile(hold_worker,NULL);return NULL;
}
static void *worker_start(void *unused) {return scm_with_guile(worker_inside,unused);}
static void *leading_loop(void *unused) {
    (void)unused;scm_c_eval_string("(let loop () (loop))");return NULL;
}
#endif
#ifdef WORKER_GUILE_FIRST
static void initialize_worker_before_leading(void) {
    pthread_t worker;if(pthread_create(&worker,NULL,worker_start,NULL))_Exit(80);
    pthread_mutex_lock(&initialized_lock);
    while(!initialized)pthread_cond_wait(&initialized_wake,&initialized_lock);
    pthread_mutex_unlock(&initialized_lock);
    FILE *ready=fopen("ready","w");if(!ready)_Exit(81);
    fputs("ready\n",ready);if(fclose(ready))_Exit(82);
    usleep(50000);
    scm_with_guile(leading_loop,NULL);
}
#endif
__attribute__((constructor)) static void construct(void) {
#ifndef WORKER_GUILE_FIRST
    scm_init_guile();
#ifdef CONSTRUCTOR_GUILE_SIGNAL
    scm_c_eval_string("(begin (call-with-output-file \"ready\" (lambda (p) (display \"ready\\n\" p))) (let loop () (loop)))");
#endif
#endif
}
static void *retained(void *data) {
    const char *request=data;
    SCM kept=scm_make_vector(scm_from_int(10000),scm_from_locale_string("kept"));
    char *reply=gmk_expand(request);
    scm_c_eval_string("(gc)");
    char *tail=scm_to_locale_string(scm_c_vector_ref(kept,5000));
    size_t left=strlen(reply),right=strlen(tail);
    char *result=gmk_alloc((unsigned int)(left+right+1));
    memcpy(result,reply,left);memcpy(result+left,tail,right+1);
    gmk_free(reply);free(tail);scm_remember_upto_here_1(kept);return result;
}
static char *invoke(const char *name,unsigned int argc,char **argv) {
    (void)name;(void)argc;(void)argv;
    return scm_with_guile(retained,"native-nested");
}
int native_guile_gmk_setup(const struct Location *location) {
    (void)location;
#ifdef WORKER_GUILE_FIRST
    initialize_worker_before_leading();
#endif
    live=1;
    gmk_add_function("native_guile",invoke,0,0,0);return 1;
}
__attribute__((destructor)) static void destruct(void) {
    if(live){
        char *value=scm_with_guile(retained,"finalizer");
        fprintf(stderr,"native-finalizer:%s\n",value);gmk_free(value);
    }
}
