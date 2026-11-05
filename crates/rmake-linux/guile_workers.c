/* SPDX-License-Identifier: GPL-3.0-or-later
 * Original isolated typed transport. No SCM object or Rust host pointer crosses
 * threads. This prototype has no frontend or production acceptance. */
#define _GNU_SOURCE
#define GC_THREADS
#define GC_NO_THREAD_REDIRECTS
#include "guile_worker_packet.h"
#include "guile_workers.h"
#include <libguile.h>
#include <gc/gc.h>
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <sys/eventfd.h>
#include <sys/syscall.h>
#include <unistd.h>

struct command {
    void *session;rg_worker_operation operation;void *reply;
    struct rg_environment environment;
};
struct abandoned {void *session;struct abandoned *next;};
struct worker {
    atomic_uint references;
    struct worker *next;
    struct worker *previous;
    struct rg_worker_packet *initial,*event;
    void *event_session;
    struct command command;
    struct abandoned *abandoned;
    int command_ready,event_ready,finished,cancelled,terminal;
    void *reply;
    struct rg_environment reply_environment,completed_environment;
    int completed_environment_valid;
};
static pthread_mutex_t mutex=PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t changed=PTHREAD_COND_INITIALIZER;
static struct worker *head,*tail;
static _Thread_local struct worker *on_worker,*origin;
static struct worker *terminal_origin;
static SCM leading_thread,notification;
static atomic_int ready;
/* The Rust scheduler wait owns this descriptor. Queue admission, notification,
 * and detachment share the same mutex so no worker can write a reused fd. */
static atomic_int wait_descriptor=-1;
static SCM service(void);
extern void *rg_dispatch_worker_host_yield(const void *request);
extern int rg_guile_leading_evaluator_active(void);

static void lock(void){if(pthread_mutex_lock(&mutex))_Exit(2);}
static void unlock(void){if(pthread_mutex_unlock(&mutex))_Exit(2);}
static void release(struct worker *worker) {
    if(atomic_fetch_sub_explicit(&worker->references,1,memory_order_acq_rel)!=1)return;
    rg_worker_packet_destroy(worker->initial);
    rg_worker_packet_destroy(worker->event);
    while(worker->abandoned) {
        struct abandoned *next=worker->abandoned->next;
        free(worker->abandoned);worker->abandoned=next;
    }
    free(worker);
}
struct waiting {struct worker *worker;int host;};
static void *wait_without_guile(void *data) {
    struct waiting *waiting=data;
    struct worker *worker=waiting->worker;
    lock();
    for(;;) {
        int complete=waiting->host?(worker->event_ready||worker->cancelled||head!=NULL)
                                  :(worker->command_ready||worker->finished);
        if(complete)break;
        if(pthread_cond_wait(&changed,&mutex))_Exit(2);
    }
    unlock();return NULL;
}
static void wait_for(struct worker *worker,int host) {
    struct waiting waiting={worker,host};
    struct rg_environment saved;
    rg_environment_capture(&saved);
    scm_without_guile(wait_without_guile,&waiting);
    rg_environment_apply(&saved);
}
static void publish(struct worker *worker,void *session,const void *request) {
    struct rg_worker_packet *packet=rg_worker_packet_copy(request);
    if(!packet)_Exit(2);
    lock();
    if(worker->event_ready)_Exit(2);
    rg_worker_packet_destroy(worker->event);
    worker->event=packet;worker->event_session=session;worker->event_ready=1;
    pthread_cond_broadcast(&changed);unlock();
}
/* A callback waits on its ordinary registered worker stack and can execute
 * nested native commands there. It never runs a Rust handler. */
static void *worker_turn(struct worker *worker,void *session) {
    for(;;) {
        wait_for(worker,0);
        lock();
        if(worker->finished) {
            if(session||worker->terminal)_Exit(2);
            void *reply=worker->reply;
            struct rg_environment environment=worker->reply_environment;
            unlock();rg_environment_apply(&environment);return reply;
        }
        struct command command=worker->command;
        worker->command_ready=0;unlock();
        rg_environment_apply(&command.environment);
        if(command.session==session) {
            lock();
            for(struct abandoned *item=worker->abandoned;item;item=item->next)
                if(item->session==session)_Exit(2);
            unlock();
            return command.reply;
        }
        if(!command.operation)_Exit(2);
        const void *event=command.operation(command.session);
        publish(worker,command.session,event);
    }
}
static void *submit_inside(void *data) {
    struct worker *worker=data;
    /* Transport waits do not permit a Scheme async to unwind a request whose
     * acknowledgement is still pending. Nested callback barriers need separate
     * qualification before this experimental transport can be enabled. */
    scm_dynwind_begin(0);scm_dynwind_block_asyncs();
    lock();
    int evaluator=rg_guile_leading_evaluator_active();
    int descriptor=atomic_load_explicit(&wait_descriptor,memory_order_relaxed);
    if(!evaluator&&descriptor<0) {
        unlock();
        static const char message[]="rmake: native API used outside an active invocation\n";
        ssize_t written=write(STDERR_FILENO,message,sizeof message-1);
        (void)written;
        _Exit(2);
    }
    if(tail)tail->next=worker;else head=worker;tail=worker;
    if(descriptor>=0) {
        int saved=errno;
        uint64_t value=1;
        ssize_t count;
        do count=write(descriptor,&value,sizeof value);while(count<0&&errno==EINTR);
        if(count!=(ssize_t)sizeof value&&(count>=0||errno!=EAGAIN))_Exit(2);
        errno=saved;
    }
    pthread_cond_broadcast(&changed);unlock();
    if(evaluator)scm_system_async_mark_for_thread(notification,leading_thread);
    (void)worker_turn(worker,NULL);
    on_worker=worker->previous;
    scm_dynwind_end();
    rg_environment_capture(&worker->completed_environment);
    worker->completed_environment_valid=1;
    return NULL;
}
void *rg_workers_submit(const void *request) {
    struct rg_worker_packet *packet=rg_worker_packet_copy(request);
    if(!packet)_Exit(2);
    struct worker *worker=calloc(1,sizeof *worker);
    if(!worker)_Exit(2);
    atomic_init(&worker->references,2);
    worker->initial=packet;
    struct worker *previous=on_worker;on_worker=worker;
    worker->previous=previous;
    rg_environment_apply(&packet->environment);
    scm_with_guile(submit_inside,worker);
    on_worker=previous;
    if(worker->completed_environment_valid)
        rg_environment_apply(&worker->completed_environment);
    lock();
    if(!worker->finished)worker->cancelled=1;
    void *reply=worker->reply;
    pthread_cond_broadcast(&changed);unlock();
    release(worker);return reply;
}
void *rg_workers_yield(void *session,const void *request) {
    if(!on_worker)_Exit(2);
    publish(on_worker,session,request);
    return worker_turn(on_worker,session);
}
static SCM service(void) {
    if(syscall(SYS_gettid)!=getpid())_Exit(2);
    lock();
    struct worker *worker=head;
    if(!worker){unlock();return SCM_UNSPECIFIED;}
    head=worker->next;if(!head)tail=NULL;
    unlock();
    struct rg_environment saved;
    rg_environment_capture(&saved);
    rg_environment_apply(&worker->initial->environment);
    struct worker *previous=origin;origin=worker;
    void *reply=rg_dispatch_worker_host_yield(&worker->initial->request);
    origin=previous;
    struct rg_environment reply_environment;
    rg_environment_capture(&reply_environment);
    rg_environment_apply(&saved);
    lock();
    if(worker->terminal)_Exit(2);
    worker->reply=reply;worker->finished=1;
    worker->reply_environment=reply_environment;
    pthread_cond_broadcast(&changed);unlock();
    release(worker);return SCM_UNSPECIFIED;
}
const void *rg_workers_invoke(void *session,rg_worker_operation operation,void *reply) {
    struct rg_environment environment;
    rg_environment_capture(&environment);
    struct worker *worker=origin?origin:terminal_origin;
    if(!worker)_Exit(2);
    lock();
    if(worker->command_ready)_Exit(2);
    worker->command=(struct command){session,operation,reply,environment};worker->command_ready=1;
    pthread_cond_broadcast(&changed);unlock();
    for(;;) {
        wait_for(worker,1);
        lock();
        if(worker->cancelled)_Exit(2);
        if(worker->event_ready) {
            if(worker->event_session!=session)_Exit(2);
            worker->event_ready=0;
            const void *event=&worker->event->request;
            environment=worker->event->environment;
            unlock();rg_environment_apply(&environment);return event;
        }
        int work=head!=NULL;unlock();
        if(work)service();
    }
}
void rg_workers_initialize(void) {
    if(atomic_load_explicit(&ready,memory_order_acquire))return;
    if(syscall(SYS_gettid)!=getpid())_Exit(2);
    leading_thread=scm_current_thread();scm_gc_protect_object(leading_thread);
    notification=scm_c_make_gsubr("rmake-worker-service",0,0,0,service);
    scm_gc_protect_object(notification);
    atomic_store_explicit(&ready,1,memory_order_release);
}
int rg_workers_can_submit(void) {
    return !on_worker&&syscall(SYS_gettid)!=getpid()
        &&atomic_load_explicit(&ready,memory_order_acquire)
        &&(rg_guile_leading_evaluator_active()
           ||atomic_load_explicit(&wait_descriptor,memory_order_relaxed)>=0)
        &&GC_thread_is_registered();
}
int rg_workers_begin_wait(void) {
    if(!atomic_load_explicit(&ready,memory_order_acquire))return -2;
    int ambient=errno;
    lock();
    if(atomic_load_explicit(&wait_descriptor,memory_order_relaxed)>=0) {
        unlock();errno=EBUSY;return -1;
    }
    int descriptor=eventfd(0,EFD_NONBLOCK|EFD_CLOEXEC);
    if(descriptor>=0&&descriptor<5) {
        int replacement=fcntl(descriptor,F_DUPFD_CLOEXEC,5);
        int saved=errno;
        close(descriptor);descriptor=replacement;errno=saved;
    }
    if(descriptor>=0)
        atomic_store_explicit(&wait_descriptor,descriptor,memory_order_relaxed);
    int saved=descriptor>=0?ambient:errno;
    unlock();errno=saved;
    return descriptor;
}
int rg_workers_wait_notification(int descriptor) {
    int saved=errno;
    lock();
    if(atomic_load_explicit(&wait_descriptor,memory_order_relaxed)!=descriptor)_Exit(2);
    uint64_t value;
    ssize_t count;
    do count=read(descriptor,&value,sizeof value);while(count<0&&errno==EINTR);
    if(count!=(ssize_t)sizeof value&&(count>=0||errno!=EAGAIN))_Exit(2);
    int pending=head!=NULL;
    unlock();errno=saved;return pending;
}
int rg_workers_end_wait(int descriptor,int idle_only) {
    lock();
    if(atomic_load_explicit(&wait_descriptor,memory_order_relaxed)!=descriptor)_Exit(2);
    if(idle_only&&head) {unlock();return 0;}
    atomic_store_explicit(&wait_descriptor,-1,memory_order_relaxed);
    unlock();return 1;
}
static void *service_wait(void *unused) {
    (void)unused;
    for(;;) {
        lock();int pending=head!=NULL;unlock();
        if(!pending)return NULL;
        service();
    }
}
void rg_workers_service_wait(void) {
    struct rg_environment saved;
    rg_environment_capture(&saved);
    scm_with_guile(service_wait,NULL);
    rg_environment_apply(&saved);
}
int rg_workers_on_worker(void){return on_worker!=NULL;}
int rg_workers_have_origin(void){return origin!=NULL||terminal_origin!=NULL;}
void rg_workers_abandon(void *session) {
    if(!origin)return;
    struct abandoned *item=malloc(sizeof *item);
    if(!item)_Exit(2);
    lock();
    item->session=session;item->next=origin->abandoned;origin->abandoned=item;
    origin->terminal=1;terminal_origin=origin;
    unlock();
}
int rg_workers_function_base(void *function,uintptr_t *base) {
    if(!origin)return 0;
    lock();
    struct rg_worker_packet *packet=origin->event;
    if(!packet||packet->request.kind!=3||packet->request.function!=function)
        packet=origin->initial;
    int matches=packet&&packet->request.kind==3&&packet->request.function==function;
    if(matches)*base=packet->function_base;
    unlock();return matches;
}
