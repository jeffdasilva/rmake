/* SPDX-License-Identifier: GPL-3.0-or-later
 * Private owned transport packet for the isolated worker-bridge prototype. */
#ifndef RMAKE_GUILE_WORKER_PACKET_H
#define RMAKE_GUILE_WORKER_PACKET_H
#include <stdint.h>
#include "guile_environment.h"

/* These kinds are mirrored by the Rust native request dispatcher. */
enum rg_guile_request_kind {
    RG_GUILE_EXPAND = 7,
    RG_GUILE_EVALUATE = 8,
};

struct rg_worker_location {const char *file;unsigned long line;};
struct rg_worker_request {
    int kind;
    const char *text;
    const struct rg_worker_location *location;
    void *function;
    unsigned int minimum,maximum,flags;
};
struct rg_worker_packet {
    struct rg_worker_request request;
    struct rg_worker_location location;
    char *text,*file;
    uintptr_t function_base;
    struct rg_environment environment;
};
struct rg_worker_packet *rg_worker_packet_copy(const void *request);
void rg_worker_packet_destroy(struct rg_worker_packet *packet);
#endif
