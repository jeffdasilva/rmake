/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original GNU-compatible ABI fixture; no GNU header or implementation used. */
#define _POSIX_C_SOURCE 200809L
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

struct Source { const char *filename; unsigned long line; };
typedef char *(*Function)(const char *, unsigned int, char **);
extern char *gmk_alloc(unsigned int);
extern void gmk_free(char *);
extern char *gmk_expand(const char *);
extern void gmk_eval(const char *, const struct Source *);
extern void gmk_add_function(const char *, Function, unsigned int, unsigned int, unsigned int);
int plugin_is_GPL_compatible = 1;
static char source[1024];

static void mark(const char *event) {
    FILE *stream = fopen("seen", "a");
    if (!stream) exit(90);
    fputs(event, stream);
    fputc('\n', stream);
    if (fclose(stream)) exit(91);
}
static char *duplicate(const char *text) {
    size_t length = strlen(text) + 1;
    char *result = gmk_alloc((unsigned int)length);
    memcpy(result, text, length);
    return result;
}
static char *arguments(const char *name, unsigned int argc, char **argv) {
    (void)name;
    size_t capacity = 32;
    for (unsigned int i = 0; i < argc; ++i) capacity += strlen(argv[i]) + 2;
    char *result = gmk_alloc((unsigned int)capacity);
    size_t used = (size_t)snprintf(result, capacity, "argc=%u", argc);
    for (unsigned int i = 0; i < argc; ++i) {
        result[used++] = '[';
        size_t length = strlen(argv[i]);
        memcpy(result + used, argv[i], length);
        used += length;
        result[used++] = ']';
    }
    result[used] = 0;
    return result;
}
static char *expand(const char *name, unsigned int argc, char **argv) {
    (void)name;
    return gmk_expand(argc ? argv[0] : "");
}
static char *evaluate(const char *name, unsigned int argc, char **argv) {
    (void)name;
    if (argc) gmk_eval(argv[0], NULL);
    return gmk_expand("$(VALUE)");
}
static char *where(const char *name, unsigned int argc, char **argv) {
    (void)name; (void)argc; (void)argv;
    return duplicate(source);
}
static char *release(const char *name, unsigned int argc, char **argv) {
    (void)name; (void)argc; (void)argv;
    char *owned = gmk_expand("$(VALUE)");
    gmk_free(owned);
    return duplicate("released");
}
static char *empty(const char *name, unsigned int argc, char **argv) {
    (void)name; (void)argc; (void)argv;
    return NULL;
}
static char *nul(const char *name, unsigned int argc, char **argv) {
    (void)name; (void)argc; (void)argv;
    char *result = gmk_alloc(8);
    memcpy(result, "ok\0tail", 8);
    return result;
}
static char *install(const char *name, unsigned int argc, char **argv) {
    (void)name; (void)argc; (void)argv;
    gmk_add_function("created", arguments, 0, 0, 0);
    return duplicate("installed");
}
static char *guard_eval(const char *name, unsigned int argc, char **argv) {
    (void)name;
    if (argc) gmk_eval(argv[0], NULL);
    mark("eval-returned");
    return gmk_expand("$(VALUE)");
}
static char *guard_eval_location(const char *name, unsigned int argc, char **argv) {
    (void)name;
    const struct Source location = {"module-eval.mk", 23};
    if (argc) gmk_eval(argv[0], &location);
    mark("eval-returned");
    return gmk_expand("$(VALUE)");
}
static char *guard_expand(const char *name, unsigned int argc, char **argv) {
    (void)name;
    char *result = gmk_expand(argc ? argv[0] : "");
    mark("expand-returned");
    return result;
}
#ifndef PROBE_VERSION
#define PROBE_VERSION 1
#endif
#define PROBE_STRING_(v) #v
#define PROBE_STRING(v) PROBE_STRING_(v)
static char *version(const char *name, unsigned int argc, char **argv) {
    (void)name; (void)argc; (void)argv;
    return duplicate("version-" PROBE_STRING(PROBE_VERSION));
}
static char *wait_fatal(const char *name, unsigned int argc, char **argv) {
    (void)name; (void)argc; (void)argv;
    FILE *started = NULL;
    for (unsigned int i = 0; i < 2000 && !started; ++i) {
        started = fopen("child-started", "r");
        if (!started) {
            const struct timespec pause = {0, 1000000};
            nanosleep(&pause, NULL);
        }
    }
    if (!started) exit(92);
    fclose(started);
    mark("callback-entered");
    FILE *entered = fopen("callback-entered", "w");
    if (!entered) exit(93);
    fputs("entered", entered);
    if (fclose(entered)) exit(94);
    const struct Source location = {"module-worker.mk", 17};
    gmk_eval("$(error native-worker-stop)", &location);
    mark("forbidden-return");
    return duplicate("");
}
#if defined(PROBE_LIFECYCLE) || defined(PROBE_CONSTRUCTOR) || defined(PROBE_CONSTRUCTOR_FATAL) || defined(PROBE_CONSTRUCTOR_NESTED)
__attribute__((constructor)) static void initialize(void) {
#ifdef PROBE_LIFECYCLE
    mark("constructor");
#else
    mark("constructor-enter");
#if defined(PROBE_CONSTRUCTOR_FATAL)
    gmk_eval("$(error constructor-stop)", NULL);
#elif defined(PROBE_CONSTRUCTOR_NESTED)
    gmk_eval("load inner.so(inner_gmk_setup)\n$(file >>seen,nested-eval)", NULL);
#else
    gmk_eval("CONSTRUCTOR_SEEN := ready", NULL);
#endif
    mark("constructor-return");
#endif
}
#endif
#ifdef PROBE_LIFECYCLE
__attribute__((destructor)) static void finalize(void) { mark("destructor"); }
#endif
static int setup(const struct Source *location) {
    snprintf(source, sizeof source, "%s:%lu", location->filename, location->line);
    gmk_add_function("probe_version", version, 0, 1, 0);
    gmk_add_function("probe_wait_fatal", wait_fatal, 0, 1, 0);
    gmk_add_function("probe_args", arguments, 0, 0, 0);
    gmk_add_function("probe_pair", arguments, 2, 2, 0);
    gmk_add_function("probe_raw", arguments, 0, 2, 1);
    gmk_add_function("probe_raw_pair", arguments, 2, 2, 1);
    gmk_add_function("probe_expand", expand, 0, 1, 0);
    gmk_add_function("probe_eval", evaluate, 0, 1, 0);
    gmk_add_function("probe_where", where, 0, 1, 0);
    gmk_add_function("probe_free", release, 0, 1, 0);
    gmk_add_function("probe_null", empty, 0, 1, 0);
    gmk_add_function("probe_nul", nul, 0, 1, 0);
    gmk_add_function("probe_install", install, 0, 1, 0);
    gmk_add_function("probe_eval_guard", guard_eval, 0, 1, 0);
    gmk_add_function("probe_eval_location", guard_eval_location, 0, 1, 0);
    gmk_add_function("probe_expand_guard", guard_expand, 0, 1, 0);
#ifdef PROBE_TRACE_SETUP
    mark("setup");
#endif
    return getenv("PROBE_SETUP_ZERO") ? 0 : 1;
}
int probe_api_gmk_setup(const struct Source *location) { return setup(location); }
int alternate_setup(const struct Source *location) { return setup(location); }

#ifdef PROBE_INNER
int inner_gmk_setup(const struct Source *location) { return setup(location); }
#endif

#ifdef PROBE_EXIT_API
__attribute__((destructor)) static void final_api(void) {
    char text[1024];
    char *expanded = gmk_expand("$(VALUE)");
    snprintf(text, sizeof text, "destructor-value=%s", expanded);
    mark(text);
    gmk_free(expanded);
    gmk_eval("FINAL_VALUE := updated", NULL);
    expanded = gmk_expand("$(FINAL_VALUE)");
    snprintf(text, sizeof text, "destructor-eval=%s", expanded);
    mark(text);
    gmk_free(expanded);
    printf("destructor-stdio\n");
    fputs("destructor-stderr\n", stderr);
}
#endif

#ifdef PROBE_EXIT_FD_REUSE
__attribute__((destructor)) static void final_fd_reuse(void) {
    FILE *stream = fopen("seen", "a");
    if (!stream) exit(96);
    if (fileno(stream) != 1) exit(97);
    char *result = gmk_expand("$(info MUST-NOT-WRITE)");
    gmk_free(result);
    fputs("destructor-fd-mark\n", stream);
    if (fclose(stream)) exit(98);
}
#endif
