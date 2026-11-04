/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original optional ordinary-C-stack dispatcher. No SCM value enters Rust. */
#define _GNU_SOURCE
#define GC_THREADS
#define GC_NO_THREAD_REDIRECTS
#include <gc/gc.h>
#include <libguile.h>
#include <errno.h>
#include <fenv.h>
#include <pthread.h>
#include <stdint.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <ucontext.h>
#include <unistd.h>

typedef const void *(*rg_operation)(void *);
struct rg_pause {
    ucontext_t context;
    struct rg_pause *outer;
    void *session;
    int abandoned;
};
struct rg_dispatcher {
    ucontext_t host;
    void *mapping;
    size_t mapping_size, guard_size, stack_size;
    void (*entry)(void *);
    void *data;
    struct rg_pause *waiting;
    rg_operation operation;
    void *session, *reply;
    const void *event;
    void *event_session;
    int returning, status, on_host;
};
static _Thread_local struct rg_dispatcher *dispatcher;
static _Thread_local fenv_t thread_environment;

static void capture_environment(void) {
    int saved=errno;
    if (fegetenv(&thread_environment)) _Exit(2);
    errno=saved;
}
static void restore_environment(void) {
    int saved=errno;
    if (fesetenv(&thread_environment)) _Exit(2);
    errno=saved;
}


static int registered(void) {
    return GC_is_init_called() && GC_thread_is_registered();
}
static void change(ucontext_t *from, ucontext_t *to) {
    /* A context is a stack, not a second operating-system thread. */
    if (pthread_sigmask(SIG_SETMASK,NULL,&to->uc_sigmask)) _Exit(2);
    capture_environment();
    if (swapcontext(from,to)<0) _Exit(2);
    restore_environment();
}
static void *swap_to_host(void *data) {
    struct rg_pause *pause=data;
    dispatcher->on_host=1;
    change(&pause->context,&dispatcher->host);
    dispatcher->on_host=0;
    return NULL;
}
static void *block_host_turn(void *data) {
    return GC_do_blocking(swap_to_host,data);
}
static void host_turn(struct rg_pause *pause) {
    /* GC changes state on the ordinary C stack using its allocator lock.
     * Rust runs inactive on a private stack containing no SCM values. */
    if (registered()) GC_call_with_gc_active(block_host_turn,pause);
    else swap_to_host(pause);
}
struct rg_invocation { rg_operation operation; void *session; const void *event; };
static void *perform(void *data) {
    struct rg_invocation *call=data;
    call->event=call->operation(call->session);
    return NULL;
}
static void perform_pending(void) {
    struct rg_dispatcher *d=dispatcher;
    struct rg_invocation call={d->operation,d->session,NULL};
    d->operation=NULL;
    if (registered()) GC_call_with_gc_active(perform,&call);
    else perform(&call);
    d->event=call.event;
    d->event_session=call.session;
}
int rg_dispatch_active(void) { return dispatcher!=NULL; }
int rg_dispatch_on_host(void) { return dispatcher && dispatcher->on_host; }
const void *rg_dispatch_call(void *session,rg_operation operation,void *reply) {
    struct rg_dispatcher *d=dispatcher;
    if (!d || !d->on_host || !d->waiting) _Exit(2);
    d->session=session;d->operation=operation;d->reply=reply;
    change(&d->host,&d->waiting->context);
    if (d->event_session!=session || !d->event) _Exit(2);
    return d->event;
}
void *rg_dispatch_yield(void *session,const void *event) {
    struct rg_dispatcher *d=dispatcher;
    if (!d || d->on_host) _Exit(2);
    struct rg_pause pause;
    pause.outer=d->waiting;pause.session=session;pause.abandoned=0;
    d->waiting=&pause;d->event=event;d->event_session=session;
    for (;;) {
        host_turn(&pause);
        if (d->returning) _Exit(d->status ? d->status : 2);
        if (d->session==session) {
            if (pause.abandoned) _Exit(2);
            d->waiting=pause.outer;
            return d->reply;
        }
        if (!d->operation) _Exit(2);
        perform_pending();
    }
}
void rg_dispatch_abandon(void *session) {
    if (!dispatcher) return;
    for (struct rg_pause *p=dispatcher->waiting;p;p=p->outer)
        if (p->session==session) p->abandoned=1;
}
static void host_entry(void) {
    restore_environment();
    struct rg_dispatcher *d=dispatcher;
    d->entry(d->data);
    d->returning=1;
    change(&d->host,&d->waiting->context);
    _Exit(2);
}
int rg_dispatch_run(void (*entry)(void *),void *data) {
    if (dispatcher || syscall(SYS_gettid)!=getpid()) { errno=EPERM;return -1; }
    struct rg_dispatcher *d=calloc(1,sizeof *d);
    if (!d) return -1;
    d->entry=entry;d->data=data;
    long page=sysconf(_SC_PAGESIZE);
    if (page<=0) { free(d);errno=EINVAL;return -1; }
    d->guard_size=(size_t)page;
    d->stack_size=256*1024;
    d->mapping_size=d->stack_size+2*d->guard_size;
    d->mapping=mmap(NULL,d->mapping_size,PROT_NONE,
                    MAP_PRIVATE|MAP_ANONYMOUS,-1,0);
    if (d->mapping==MAP_FAILED) { int saved=errno;free(d);errno=saved;return -1; }
    if (mprotect((char *)d->mapping+d->guard_size,d->stack_size,
                 PROT_READ|PROT_WRITE)<0 || getcontext(&d->host)<0) {
        int saved=errno;munmap(d->mapping,d->mapping_size);free(d);errno=saved;return -1;
    }
    d->host.uc_stack.ss_sp=(char *)d->mapping+d->guard_size;
    d->host.uc_stack.ss_size=d->stack_size;d->host.uc_link=NULL;
    makecontext(&d->host,host_entry,0);
    dispatcher=d;
    struct rg_pause root;
    root.outer=NULL;root.session=NULL;root.abandoned=0;d->waiting=&root;
    for (;;) {
        host_turn(&root);
        if (d->returning) break;
        if (!d->operation) _Exit(2);
        perform_pending();
    }
    dispatcher=NULL;
    munmap(d->mapping,d->mapping_size);free(d);return 0;
}

/* Component evaluator: string-valued results only. Language conversions and
 * public registration require independent GNU2 qualification before exposure. */
extern char *gmk_expand(const char *);
extern void gmk_eval(const char *,const void *);
extern void gmk_free(char *);
extern void rm_native_set_language_error(const char *);
extern int rg_guile_prepare_interrupts(void);
extern int rg_guile_pending_signal(void);
extern void rm_native_interrupt_request(int);
static int module_ready;
static atomic_int guile_entered;
static SCM make_expand(SCM value) {
    char *text=scm_to_locale_string(value);
    char *expanded=gmk_expand(text);free(text);
    SCM result=scm_from_locale_string(expanded);gmk_free(expanded);return result;
}
static SCM make_evaluate(SCM value) {
    char *text=scm_to_locale_string(value);
    gmk_eval(text,NULL);free(text);return scm_from_locale_string("");
}
static void define_module(void *unused) {
    (void)unused;
    scm_c_define_gsubr("gmk-expand",1,0,0,make_expand);
    scm_c_define_gsubr("gmk-eval",1,0,0,make_evaluate);
    scm_c_export("gmk-expand","gmk-eval",NULL);
}
struct rg_evaluation { const char *source; char *result; };
static SCM evaluate_body(void *data) {
    struct rg_evaluation *evaluation=data;
    if (!module_ready) {
        scm_c_define_module("gnu make",define_module,NULL);
        scm_c_use_module("gnu make");module_ready=1;
    }
    SCM result=scm_c_eval_string(evaluation->source);
    evaluation->result=scm_to_locale_string(result);
    return SCM_UNSPECIFIED;
}
static SCM caught(void *unused,SCM tag,SCM arguments) {
    (void)unused;SCM port=scm_open_output_string();
    scm_print_exception(port,SCM_BOOL_F,tag,arguments);
    char *message=scm_to_utf8_string(scm_get_output_string(port));
    rm_native_set_language_error(message);free(message);return SCM_UNSPECIFIED;
}
static void *evaluate_inside(void *data) {
    scm_c_catch(SCM_BOOL_T,evaluate_body,data,caught,NULL,NULL,NULL);return NULL;
}
char *rg_guile_callback(const char *name,unsigned int argc,char **argv) {
    (void)name;
    if (!argc || !argv[0][0]) return NULL;
    atomic_store_explicit(&guile_entered,1,memory_order_release);
    if (rg_guile_pending_signal()) {
        rm_native_interrupt_request(rg_guile_pending_signal());
        abort();
    }
    if (rg_guile_prepare_interrupts()<0) {
        rm_native_set_language_error("unable to reserve Guile signal delivery");
        return NULL;
    }
    struct rg_evaluation evaluation={argv[0],NULL};
    scm_with_guile(evaluate_inside,&evaluation);
    return evaluation.result;
}
int rg_guile_initialized(void) {
    return atomic_load_explicit(&guile_entered,memory_order_acquire);
}
