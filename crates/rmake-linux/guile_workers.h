/* SPDX-License-Identifier: GPL-3.0-or-later
 * Isolated prototype only: typed worker requests never contain Rust or SCM. */
#ifndef RMAKE_GUILE_WORKERS_H
#define RMAKE_GUILE_WORKERS_H
#include <stdint.h>
typedef const void *(*rg_worker_operation)(void *);
void rg_workers_initialize(void);
int rg_workers_can_submit(void);
int rg_workers_on_worker(void);
int rg_workers_have_origin(void);
void *rg_workers_submit(const void *request);
void *rg_workers_yield(void *session,const void *request);
const void *rg_workers_invoke(void *session,rg_worker_operation operation,void *reply);
void rg_workers_abandon(void *session);
int rg_workers_function_base(void *function,uintptr_t *base);
int rg_workers_begin_wait(void);
int rg_workers_wait_notification(int descriptor);
int rg_workers_end_wait(int descriptor,int idle_only);
void rg_workers_service_wait(void);
#endif
