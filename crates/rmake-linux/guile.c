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
#include "guile_workers.h"
#include "guile_worker_packet.h"
#include "guile_environment.h"

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
    void *current_session;
};
static _Thread_local struct rg_dispatcher *dispatcher;
static _Thread_local struct rg_environment thread_environment;

static void capture_environment(void) {
    rg_environment_capture(&thread_environment);
}
static void restore_environment(void) {
    rg_environment_apply(&thread_environment);
}

static int registered(void) {
    return GC_is_init_called() && GC_thread_is_registered();
}
static void change(ucontext_t *from, ucontext_t *to) {
    /* A context is a stack, not a second operating-system thread. */
    capture_environment();
    to->uc_sigmask=thread_environment.mask;
    if (swapcontext(from,to)<0) _Exit(2);
    restore_environment();
}
static void *swap_to_host(void *data) {
    struct rg_pause *pause=data;
    dispatcher->on_host=1;
    restore_environment();
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
    capture_environment();
    if (registered()) GC_call_with_gc_active(block_host_turn,pause);
    else swap_to_host(pause);
    restore_environment();
}
struct rg_invocation { rg_operation operation; void *session; const void *event; };
static void *perform(void *data) {
    struct rg_invocation *call=data;
    restore_environment();
    call->event=call->operation(call->session);
    capture_environment();
    return NULL;
}
static void perform_pending(void) {
    struct rg_dispatcher *d=dispatcher;
    struct rg_invocation call={d->operation,d->session,NULL};
    d->operation=NULL;
    void *previous=d->current_session;
    d->current_session=call.session;
    if(rg_workers_have_origin())
        call.event=rg_workers_invoke(call.session,call.operation,d->reply);
    else {
        capture_environment();
        if (registered()) GC_call_with_gc_active(perform,&call);
        else perform(&call);
        restore_environment();
    }
    d->current_session=previous;
    d->event=call.event;
    d->event_session=call.session;
}
int rg_dispatch_active(void) { return dispatcher!=NULL||rg_workers_on_worker(); }
int rg_dispatch_on_host(void) { return dispatcher && dispatcher->on_host; }
_Noreturn void rg_dispatch_abort(void) {
    static const char message[]="rmake: panic in Guile host\n";
    /* No stdio lock, language callback, finalizer, or retained Rust destructor
     * may run after a panic starts unwinding the private interpreter stack. */
    ssize_t written=write(STDERR_FILENO,message,sizeof message-1);
    (void)written;
    _Exit(101);
}
const void *rg_dispatch_call(void *session,rg_operation operation,void *reply) {
    struct rg_dispatcher *d=dispatcher;
    if (!d || !d->on_host || !d->waiting) _Exit(2);
    d->session=session;d->operation=operation;d->reply=reply;
    change(&d->host,&d->waiting->context);
    if (d->event_session!=session || !d->event) _Exit(2);
    return d->event;
}
void *rg_dispatch_yield(void *session,const void *event) {
    if(rg_workers_on_worker())return rg_workers_yield(session,event);
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
    rg_workers_abandon(session);
    for (struct rg_pause *p=dispatcher->waiting;p;p=p->outer)
        if (p->session==session) p->abandoned=1;
}
void *rg_dispatch_worker_host_yield(const void *event) {
    if(!dispatcher||!dispatcher->current_session)_Exit(2);
    return rg_dispatch_yield(dispatcher->current_session,event);
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

/* Scheme objects stay in collector-visible C locals/Guile pairs throughout
 * conversion. The explicit work stack avoids recursion on the native C stack. */
extern char *rm_guile_expand(const char *);
extern void rm_guile_evaluate(const char *);
extern void gmk_free(char *);
extern void rm_native_set_language_error(const char *);
extern int rg_guile_prepare_interrupts(void);
extern int rg_guile_pending_signal(void);
extern void rm_native_interrupt_request(int);
static int module_ready;
static SCM scalar_binding;
static SCM default_scalar_converter;
static SCM render_number_or_symbol;
static SCM finish_conversion;
static atomic_int guile_entered;
static atomic_uint leading_evaluations;
int rg_guile_leading_evaluator_active(void) {
    return atomic_load_explicit(&leading_evaluations,memory_order_acquire)!=0;
}
static SCM scalar_text(SCM value) {
    if (scm_is_false(value) || scm_is_null(value) || scm_is_eq(value, SCM_UNSPECIFIED))
        return SCM_BOOL_F;
    if (scm_is_eq(value, SCM_BOOL_T))
        return scm_from_utf8_string("#t");
    if (scm_is_symbol(value) || scm_is_number(value))
        return scm_call_1(render_number_or_symbol, value);
    if (scm_is_true(scm_char_p(value)))
        return scm_make_string(scm_from_int(1), value);
    if (scm_is_string(value)) {
        if (!scm_c_string_length(value))
            return SCM_BOOL_F;
        if (scm_is_true(scm_string_every(scm_char_set_printing, value, SCM_UNDEFINED, SCM_UNDEFINED)))
            return value;
    } else if (scm_is_true(scm_variable_p(value))) {
        return SCM_BOOL_F;
    }
    /* Use Guile's ordinary exception machinery; the public and component
     * entry points deliberately choose different outer error consumers. */
    return scm_call_2(scm_c_public_ref("guile","error"),
                      scm_from_locale_string("Unknown object:"),value);
}
static SCM convert_scalar(SCM value) {
    /* Scheme code can rebind the converter, even while walking a list. */
    SCM converter = scm_variable_ref(scalar_binding);
    if (scm_is_eq(converter, default_scalar_converter))
        return scalar_text(value);
    return scm_call_1(converter, value);
}
static SCM object_text(SCM value) {
    if (!scm_is_pair(value)) {
        SCM text = convert_scalar(value);
        SCM parts = scm_is_false(text) ? SCM_EOL : scm_list_1(text);
        return scm_call_1(finish_conversion, parts);
    }
    const SCM visit = SCM_BOOL_F;
    const SCM follow_tail = SCM_UNSPECIFIED;
    const SCM leave_pair = SCM_BOOL_T;
    SCM work=scm_list_1(scm_cons(visit,value));
    SCM active_pairs=scm_make_hash_table(scm_from_int(31));
    SCM parts=SCM_EOL;
    unsigned int ticks=0;
    while(scm_is_pair(work)) {
        SCM frame = work;
        SCM item = scm_car(frame);
        SCM phase = scm_car(item);
        SCM current = scm_cdr(item);
        work = scm_cdr(frame);
        if (scm_is_eq(phase, follow_tail)) {
            // The Scheme converter can mutate this pair while visiting its car.
            // Read the live cdr now and reuse this frame for leaving the pair.
            scm_set_car_x(item, leave_pair);
            work = scm_cons(scm_cons(visit, scm_cdr(current)), frame);
            continue;
        }
        if ((ticks++ & 63) == 0)
            scm_async_tick();
        if (scm_is_eq(phase, leave_pair)) {
            scm_hashq_remove_x(active_pairs, current);
            continue;
        }
        if(scm_is_pair(current)) {
            if(scm_is_true(scm_hashq_ref(active_pairs,current,SCM_BOOL_F)))
                scm_misc_error("guile","circular value conversion",SCM_EOL);
            scm_hashq_set_x(active_pairs,current,SCM_BOOL_T);
            work = scm_cons(scm_cons(follow_tail, current), work);
            work = scm_cons(scm_cons(visit, scm_car(current)), work);
        }else {
            SCM text=convert_scalar(current);
            if (scm_is_true(text))
                parts = scm_cons(text, parts);
        }
    }
    SCM text = scm_call_1(finish_conversion, parts);
    scm_remember_upto_here_2(work,active_pairs);
    return text;
}
static char *make_text(SCM value) {
    return scm_to_locale_string(object_text(value));
}
static SCM make_expand(SCM value) {
    char *text=make_text(value);
    char *expanded;
    if(rg_workers_can_submit()) {
        struct rg_worker_request request={.kind=RG_GUILE_EXPAND,.text=text};
        expanded=rg_workers_submit(&request);
    }else expanded=rm_guile_expand(text);
    free(text);
    SCM result=scm_from_locale_string(expanded);gmk_free(expanded);return result;
}
static SCM make_evaluate(SCM value) {
    char *text=make_text(value);
    if(rg_workers_can_submit()) {
        struct rg_worker_request request={.kind=RG_GUILE_EVALUATE,.text=text};
        (void)rg_workers_submit(&request);
    }else rm_guile_evaluate(text);
    free(text);return SCM_BOOL_F;
}
static void define_module(void *unused) {
    (void)unused;
    /* Preserve first-use resolution and later assignment to the renderer. */
    render_number_or_symbol = scm_c_eval_string(
        "(lambda (value) (object->string value))");
    // Guile resolves the lexical references when the closure first uses
    // them. Keep that resolution and select join before reverse can run.
    finish_conversion = scm_c_eval_string(
        "(lambda (parts)"
        "  (let* ((join-procedure string-join)"
        "         (ordered (reverse! parts)))"
        "    (join-procedure ordered)))");
    scm_c_define_gsubr("gmk-expand",1,0,0,make_expand);
    scm_c_define_gsubr("gmk-eval",1,0,0,make_evaluate);
    scm_c_define_gsubr("obj-to-str", 1, 0, 0, object_text);
    default_scalar_converter = scm_c_define_gsubr("to-string-maybe", 1, 0, 0, scalar_text);
    scalar_binding = scm_c_lookup("to-string-maybe");
    /* Publish a Scheme procedure with the module's live operators. Name
     * conversion can rebind them, so select them before calling the converter. */
    scm_c_eval_string(
        "(define (gmk-var v)"
        "  (let* ((expand gmk-expand)"
        "         (render format)"
        "         (name (obj-to-str v))"
        "         (reference (render #f \"$(~a)\" name)))"
        "    (expand reference)))");
    scm_c_export("gmk-expand", "gmk-eval", "gmk-var", NULL);
}
struct rg_evaluation { const char *source; char *result; int utf8_source; };
static SCM evaluate_body(void *data) {
    struct rg_evaluation *evaluation=data;
    if (!module_ready) {
        scm_c_define_module("gnu make",define_module,NULL);
        scm_c_use_module("gnu make");module_ready=1;
    }
    rg_workers_initialize();
    /* GNU-qualified empty calls initialize the language but evaluate no code. */
    if (!evaluation->source[0]) return SCM_UNSPECIFIED;
    /* Make's public Scheme source uses UTF-8 even under a C locale. */
    SCM result=evaluation->utf8_source
        ?scm_eval_string(scm_from_utf8_string(evaluation->source))
        :scm_c_eval_string(evaluation->source);
    evaluation->result=make_text(result);
    scm_remember_upto_here_1(result);
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
static void *evaluate_public(void *data) { evaluate_body(data);return NULL; }
static char *evaluate_argument(unsigned int argc,char **argv,int capture) {
    atomic_store_explicit(&guile_entered,1,memory_order_release);
    if (rg_guile_pending_signal()) {
        rm_native_interrupt_request(rg_guile_pending_signal());
        abort();
    }
    if (rg_guile_prepare_interrupts()<0) {
        rm_native_set_language_error("unable to reserve Guile signal delivery");
        return NULL;
    }
    struct rg_evaluation evaluation={.source=argc?argv[0]:"",.utf8_source=!capture};
    int leading=syscall(SYS_gettid)==getpid();
    if(leading)atomic_fetch_add_explicit(&leading_evaluations,1,memory_order_release);
    scm_with_guile(capture?evaluate_inside:evaluate_public,&evaluation);
    if(leading)atomic_fetch_sub_explicit(&leading_evaluations,1,memory_order_release);
    return evaluation.result;
}
char *rg_guile_callback(const char *name,unsigned int argc,char **argv) {
    (void)name;return evaluate_argument(argc,argv,1);
}
char *rg_guile_public_callback(const char *name,unsigned int argc,char **argv) {
    (void)name;return evaluate_argument(argc,argv,0);
}
int rg_guile_initialized(void) {
    return atomic_load_explicit(&guile_entered,memory_order_acquire);
}
