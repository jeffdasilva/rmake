/* SPDX-License-Identifier: GPL-3.0-or-later
 * C owns the opaque libc objects and their lifetimes throughout transport. */
#define _GNU_SOURCE
#include "guile_environment.h"
#include <errno.h>
#include <pthread.h>
#include <stdlib.h>
#include <string.h>
void rg_environment_capture(struct rg_environment *environment) {
    int saved=errno;
    /* Some public state types contain padding/kernel-unused words. */
    memset(environment,0,sizeof *environment);
    if(fegetenv(&environment->floating)
       ||pthread_sigmask(SIG_SETMASK,NULL,&environment->mask))_Exit(2);
    environment->locale=uselocale((locale_t)0);
    if(!environment->locale)_Exit(2);
    environment->error=saved;errno=saved;
}
void rg_environment_apply(const struct rg_environment *environment) {
    if(!uselocale(environment->locale)
       ||pthread_sigmask(SIG_SETMASK,&environment->mask,NULL)
       ||fesetenv(&environment->floating))_Exit(2);
    errno=environment->error;
}
