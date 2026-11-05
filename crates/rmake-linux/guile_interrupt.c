/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original optional public-API signal bridge. No SCM enters a C signal handler. */
#define _GNU_SOURCE
#include <libguile.h>
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <signal.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/syscall.h>

_Static_assert(ATOMIC_INT_LOCK_FREE==2,"signal bridge requires lock-free int atomics");
static atomic_int notification_writer=ATOMIC_VAR_INIT(-1);
static atomic_int pending_signal;
static pthread_t delivery;
static pthread_mutex_t ready_lock=PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t ready_wake=PTHREAD_COND_INITIALIZER;
static int notification_reader=-1,started;
static atomic_int leading_acknowledged;
static SCM interrupt_procedure;
static SCM interrupt_main(void);
extern void rm_native_interrupt_request(int);
static void wake_writer(int descriptor) {
    unsigned char byte=1;
    while(write(descriptor,&byte,1)<0&&errno==EINTR){}
}

/* Only lock-free atomics and write occur here. The host's actual signal guard
 * retains ownership of its pending flag and handler restoration policy. */
void rg_guile_notify_signal(int number) {
    int saved=errno,empty=0;
    if(atomic_compare_exchange_strong_explicit(&pending_signal,&empty,number,
                                               memory_order_release,memory_order_relaxed)) {
        int descriptor=atomic_load_explicit(&notification_writer,memory_order_acquire);
        if(descriptor>=0)wake_writer(descriptor);
    }
    errno=saved;
}
/* Component-test handler, not an installed frontend policy. */
void rg_guile_component_signal(int number) {
    int saved=errno;
    signal(number,SIG_DFL);
    rg_guile_notify_signal(number);
    errno=saved;
}
int rg_guile_pending_signal(void) {
    return atomic_load_explicit(&pending_signal,memory_order_acquire);
}
static void *brief_wait(void *unused) { (void)unused;usleep(1000);return NULL; }
static void *retain_stack(void *unused) {
    (void)unused;pthread_mutex_lock(&ready_lock);
    for(;;)pthread_cond_wait(&ready_wake,&ready_lock);
    return NULL;
}
static void *queue_interrupt(void *unused) {
    (void)unused;
    interrupt_procedure=scm_c_make_gsubr("rmake-host-signal",0,0,0,interrupt_main);
    scm_gc_protect_object(interrupt_procedure);
    SCM queued=SCM_EOL;
    /* A constructor may initialize Guile before the builtin is ever called,
     * or a worker may initialize it before the leading thread registers. Use
     * only public thread objects and a leading-TID guard, never private layout. */
    for(unsigned int attempt=0;attempt<5000&&!atomic_load_explicit(&leading_acknowledged,memory_order_acquire);attempt++) {
        SCM threads=scm_all_threads();
        for(SCM walk=threads;scm_is_pair(walk);walk=scm_cdr(walk)) {
            SCM thread=scm_car(walk);
            /* Guile can publish an in-progress registration as #f. A later
             * snapshot can notify the real thread after its handle exists. */
            if(scm_is_true(scm_thread_p(thread))&&scm_is_false(scm_memq(thread,queued))) {
                scm_system_async_mark_for_thread(interrupt_procedure,thread);
                queued=scm_cons(thread,queued);
            }
        }
        scm_without_guile(brief_wait,NULL);
    }
    scm_remember_upto_here_1(queued);
    /* This thread may have won the initial Guile/GC registration race. Keep
     * its ordinary registered C stack until terminal signal death; normal
     * Scheme overrides never invoke it. No SCM object enters Rust. */
    scm_without_guile(retain_stack,NULL);
    return NULL;
}
static void *deliver(void *unused) {
    (void)unused;
    unsigned char byte;
    for(;;){
        ssize_t count=read(notification_reader,&byte,1);
        if(count==1)break;
        if(count<0&&errno==EINTR)continue;
        _Exit(2);
    }
    /* Registration happens only on a terminal host signal. During ordinary
     * evaluation this control thread is absent from Scheme all-threads. */
    scm_with_guile(queue_interrupt,NULL);
    return NULL;
}
static int private_descriptor(int descriptor) {
    if(descriptor>=5)return descriptor;
    int replacement=fcntl(descriptor,F_DUPFD_CLOEXEC,5);
    if(replacement>=0)close(descriptor);
    return replacement;
}
int rg_guile_prepare_interrupts(void) {
    if(started)return 0;
    int descriptors[2];
    if(pipe2(descriptors,O_CLOEXEC)<0)return -1;
    int reader=private_descriptor(descriptors[0]);
    if(reader<0){int saved=errno;close(descriptors[0]);close(descriptors[1]);errno=saved;return -1;}
    int writer=private_descriptor(descriptors[1]);
    if(writer<0){int saved=errno;close(reader);close(descriptors[1]);errno=saved;return -1;}
    if(fcntl(writer,F_SETFL,O_NONBLOCK)<0){int saved=errno;close(reader);close(writer);errno=saved;return -1;}
    notification_reader=reader;
    int error=pthread_create(&delivery,NULL,deliver,NULL);
    if(error){close(reader);close(writer);notification_reader=-1;errno=error;return -1;}
    started=1;
    atomic_store_explicit(&notification_writer,writer,memory_order_release);
    if(rg_guile_pending_signal())wake_writer(writer);
    return 0;
}
static SCM interrupt_main(void) {
    if(syscall(SYS_gettid)!=getpid())return SCM_UNSPECIFIED;
    atomic_store_explicit(&leading_acknowledged,1,memory_order_release);
    rm_native_interrupt_request(rg_guile_pending_signal());
    abort();
}
