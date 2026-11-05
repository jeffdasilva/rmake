/* SPDX-License-Identifier: GPL-3.0-or-later
 * Original public-API fixture for a thread whose registration is incomplete. */
#define _GNU_SOURCE
#define GC_NO_THREAD_REDIRECTS
#include <dlfcn.h>
#include <libguile.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <unistd.h>

static atomic_int injected;

SCM scm_all_threads(void) {
    SCM (*original)(void)=dlsym(RTLD_NEXT,"scm_all_threads");
    if(!original)_Exit(92);
    SCM threads=original();
    if(access("ready",F_OK)==0&&!atomic_exchange_explicit(&injected,1,memory_order_relaxed)) {
        static const char marker[]="transient Guile thread handle supplied\n";
        ssize_t written=write(STDERR_FILENO,marker,sizeof marker-1);
        if(written!=(ssize_t)(sizeof marker-1))_Exit(93);
        return scm_cons(SCM_BOOL_F,threads);
    }
    return threads;
}
