/* SPDX-License-Identifier: GPL-3.0-or-later
 * Original owned transport; no SCM or Rust pointer is stored in a packet. */
#define _GNU_SOURCE
#include "guile_worker_packet.h"
#include <errno.h>
#include <dlfcn.h>
#include <stdlib.h>
#include <string.h>

void rg_worker_packet_destroy(struct rg_worker_packet *packet) {
    if(!packet)return;
    free(packet->text);free(packet->file);free(packet);
}
struct rg_worker_packet *rg_worker_packet_copy(const void *request) {
    if(!request){errno=EINVAL;return NULL;}
    struct rg_environment environment;
    rg_environment_capture(&environment);
    struct rg_worker_packet *packet=calloc(1,sizeof *packet);
    if(!packet)return NULL;
    packet->environment=environment;
    /* The shim's request has a different private C tag. Copy representation
     * bytes rather than violating strict aliasing by dereferencing that tag. */
    memcpy(&packet->request,request,sizeof packet->request);
    const char *text=NULL;
    const struct rg_worker_location *location=NULL;
    if(packet->request.kind==1||packet->request.kind==2
       ||packet->request.kind==3||packet->request.kind==5)
        text=packet->request.text;
    if(packet->request.kind==2)location=packet->request.location;
    /* Completion/interrupt records may retain irrelevant pointers to an older
     * API call whose C frame already returned. Never inspect those fields. */
    packet->request.text=NULL;packet->request.location=NULL;
    if(packet->request.kind!=3)packet->request.function=NULL;
    /* Registration may run inside a dlopen constructor. Resolve on the caller
     * holding the loader lock, never on the separate Rust host thread. */
    if(packet->request.kind==3&&packet->request.function) {
        Dl_info image;
        if(dladdr(packet->request.function,&image))
            packet->function_base=(uintptr_t)image.dli_fbase;
    }
    if(text) {
        packet->text=strdup(text);
        if(!packet->text)goto failed;
        packet->request.text=packet->text;
    }
    if(location) {
        memcpy(&packet->location,location,sizeof packet->location);
        if(packet->location.file) {
            packet->file=strdup(packet->location.file);
            if(!packet->file)goto failed;
            packet->location.file=packet->file;
        }
        packet->request.location=&packet->location;
    }
    rg_environment_apply(&environment);
    return packet;
failed:
    {int saved=errno;rg_worker_packet_destroy(packet);errno=saved;}
    return NULL;
}
