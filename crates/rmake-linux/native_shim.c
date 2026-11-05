/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Original Linux foreign-call boundary; no GNU implementation/header included.
 * Plugin code and all exported API wrappers run solely on this C stack. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <locale.h>
#include <fenv.h>
#include <signal.h>
#include <pthread.h>
#include <stdint.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <ucontext.h>
#include <unistd.h>

struct rm_location { const char *file; unsigned long line; };
typedef char *(*rm_callback)(const char *, unsigned int, char **);
struct rm_request {
    int kind;
    const char *text;
    const struct rm_location *location;
    void *function;
    unsigned int minimum, maximum, flags;
};
struct rm_session {
    ucontext_t host, foreign;
    void *mapping;
    size_t mapping_size, guard_size, stack_size;
    int operation, done, number;
    void *function, *result;
    char *name, *error;
    struct rm_location location;
    unsigned int argc;
    char **argv;
    struct rm_request request;
    void *reply;
    struct rm_session *abandoned_next;
};
static _Thread_local struct rm_session *active;
static _Thread_local struct rm_session *abandoned;
/* A native invocation changes stacks within one operating-system thread.
 * ucontext restores the destination stack's old floating environment unless
 * we explicitly carry the latest thread state across each boundary. */
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
#ifdef RMAKE_GUILE_DISPATCH
typedef const void *(*rg_operation)(void *);
extern int rg_dispatch_active(void);
extern int rg_dispatch_on_host(void);
extern const void *rg_dispatch_call(void *,rg_operation,void *);
extern void *rg_dispatch_yield(void *,const void *);
extern void rg_dispatch_abandon(void *);
extern int rg_guile_prepare_interrupts(void);
#endif

static struct rm_session *current(void) {
    if (!active) {
        fputs("rmake: native API used outside an active invocation\n", stderr);
        _Exit(2);
    }
    return active;
}
static void yield_request(struct rm_session *s, int kind) {
    s->request.kind = kind;
#ifdef RMAKE_GUILE_DISPATCH
    if (rg_dispatch_active()) {
        s->reply=rg_dispatch_yield(s,&s->request);
        return;
    }
#endif
    /* Signal masks describe the same operating-system thread across stacks. */
    sigprocmask(SIG_SETMASK, NULL, &s->host.uc_sigmask);
    capture_environment();
    if (swapcontext(&s->foreign, &s->host) < 0) _Exit(2);
    restore_environment();
}
char *gmk_alloc(unsigned int length) {
    char *value = malloc(length ? length : 1);
    if (!value) yield_request(current(), 4);
    return value;
}
void gmk_free(char *value) { free(value); }
char *gmk_expand(const char *text) {
    struct rm_session *s = current();
    s->request.text = text;
    yield_request(s, 1);
    return s->reply;
}
void gmk_eval(const char *text, const struct rm_location *location) {
    struct rm_session *s = current();
    s->request.text = text;
    s->request.location = location;
    yield_request(s, 2);
}
void gmk_add_function(const char *name, rm_callback function,
                      unsigned int minimum, unsigned int maximum,
                      unsigned int flags) {
    struct rm_session *s = current();
    s->request.text = name;
    s->request.function = (void *)function;
    s->request.minimum = minimum;
    s->request.maximum = maximum;
    s->request.flags = flags;
    yield_request(s, 3);
}
static void perform_operation(struct rm_session *s) {
    if (s->operation <= 1) dlerror();
    switch (s->operation) {
    case 0: s->result = dlopen(s->name, s->number); break;
    case 1: s->result = dlsym(s->function, s->name); break;
    case 2: s->number = ((int (*)(const struct rm_location *))s->function)(&s->location); break;
    case 3: s->result = ((rm_callback)s->function)(s->name, s->argc, s->argv); break;
    case 4: s->number = dlclose(s->function); break;
    case 5: exit(s->number);
#ifdef RMAKE_GUILE_DISPATCH
    case 6: {
        sigset_t mask;
        if (signal(s->number,SIG_DFL)==SIG_ERR || sigemptyset(&mask)
            ||sigaddset(&mask,s->number)||sigprocmask(SIG_UNBLOCK,&mask,NULL)) _Exit(2);
        if (kill(getpid(),s->number)) _Exit(2);
        _Exit(128+s->number);
    }
#endif
    default: abort();
    }
    if (s->operation <= 1 && !s->result) {
        const char *error = dlerror();
        s->error = error ? strdup(error) : NULL;
    }
    s->done = 1;
}
static void entry(void) {
    /* A newly created context has no resumed swapcontext call of its own. */
    restore_environment();
    struct rm_session *s = current();
    perform_operation(s);
    yield_request(s, 0);
    abort();
}
#ifdef RMAKE_GUILE_DISPATCH
static const void *main_entry(void *data) {
    struct rm_session *s=data;
    struct rm_session *previous=active;
    active=s;
    if (s->operation!=5&&s->operation!=6&&rg_guile_prepare_interrupts()<0) {
        s->request.kind=4;
        active=previous;
        return &s->request;
    }
    perform_operation(s);
    s->request.kind=0;
    active=previous;
    return &s->request;
}
void rm_native_set_language_error(const char *message) {
    struct rm_session *s=current();
    char *copy=strdup(message);
    if (!copy) { yield_request(s,4);abort(); }
    free(s->error);s->error=copy;
}
void rm_native_interrupt_request(int number) {
    struct rm_session *s=current();
    s->request.minimum=(unsigned int)number;
    yield_request(s,6);
    abort();
}
#endif
void rm_native_destroy(struct rm_session *s) {
    if (!s) return;
    if (s->mapping) munmap(s->mapping, s->mapping_size);
    if (s->argv) {
        for (unsigned int i = 0; i < s->argc; ++i) free(s->argv[i]);
    }
    free(s->argv);
    free(s->name);
    free((char *)s->location.file);
    free(s->error);
    free(s);
}
void rm_native_stdio_initialize(void);
struct rm_session *rm_native_create(int operation, void *function,
                                    const char *name, int number,
                                    const char *file, unsigned long line,
                                    unsigned int argc, const char *const *argv) {
    rm_native_stdio_initialize();
    struct rm_session *s = calloc(1, sizeof *s);
    if (!s) return NULL;
    s->operation = operation; s->function = function; s->number = number;
    s->location.line = line; s->argc = argc;
    if (name && !(s->name = strdup(name))) goto failed;
    if (file && !(s->location.file = strdup(file))) goto failed;
    if (argc) {
        s->argv = calloc((size_t)argc + 1, sizeof *s->argv);
        if (!s->argv) goto failed;
        for (unsigned int i = 0; i < argc; ++i) {
            s->argv[i] = strdup(argv[i]);
            if (!s->argv[i]) goto failed;
        }
    } else {
        s->argv = calloc(1, sizeof *s->argv);
        if (!s->argv) goto failed;
    }
#ifdef RMAKE_GUILE_DISPATCH
    if (rg_dispatch_active()) return s;
#endif
    long page = sysconf(_SC_PAGESIZE);
    if (page <= 0) goto failed;
    size_t stack = 8 * 1024 * 1024;
    struct rlimit limit;
    if (!getrlimit(RLIMIT_STACK, &limit) && limit.rlim_cur != RLIM_INFINITY
        && limit.rlim_cur > stack) stack = (size_t)limit.rlim_cur;
    if (stack > SIZE_MAX - 3 * (size_t)page) goto failed;
    stack = (stack + (size_t)page - 1) / (size_t)page * (size_t)page;
    s->mapping_size = stack + 2 * (size_t)page;
    s->mapping = mmap(NULL, s->mapping_size, PROT_NONE,
                      MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (s->mapping == MAP_FAILED) { s->mapping = NULL; goto failed; }
    void *usable = (char *)s->mapping + page;
    if (mprotect(usable, stack, PROT_READ | PROT_WRITE) < 0) goto failed;
    s->guard_size = (size_t)page;
    s->stack_size = stack;
    if (getcontext(&s->foreign) < 0) goto failed;
    /* getcontext is returns_twice: keep surviving configuration in the owned
     * session, rather than modified automatic variables across that boundary. */
    s->foreign.uc_stack.ss_sp = (char *)s->mapping + s->guard_size;
    s->foreign.uc_stack.ss_size = s->stack_size;
    s->foreign.uc_link = &s->host;
    makecontext(&s->foreign, entry, 0);
    return s;
failed:
    { int saved = errno; rm_native_destroy(s); errno = saved ? saved : ENOMEM; }
    return NULL;
}
const struct rm_request *rm_native_drive(struct rm_session *s, void *reply) {
    if (s->done) return &s->request;
#ifdef RMAKE_GUILE_DISPATCH
    if (rg_dispatch_active()) {
        if (!rg_dispatch_on_host()) _Exit(2);
        return rg_dispatch_call(s,main_entry,reply);
    }
#endif
    s->reply = reply;
    struct rm_session *previous = active;
    active = s;
    sigprocmask(SIG_SETMASK, NULL, &s->foreign.uc_sigmask);
    capture_environment();
    if (swapcontext(&s->host, &s->foreign) < 0) {
        s->request.kind = 4;
    }
    restore_environment();
    active = previous;
    return &s->request;
}
void *rm_native_result(const struct rm_session *s) { return s->result; }
int rm_native_number(const struct rm_session *s) { return s->number; }
const char *rm_native_error(const struct rm_session *s) { return s->error; }
/* On fatal interpretation, foreign automatic objects remain alive until process
 * exit. This retained stack is deliberately never resumed or unmapped. */
void rm_native_abandon(struct rm_session *s) {
#ifdef RMAKE_GUILE_DISPATCH
    rg_dispatch_abandon(s);
#endif
    s->abandoned_next = abandoned;
    abandoned = s;
}

void *rm_native_base(void *address) {
    Dl_info info;
    return dladdr(address, &info) ? info.dli_fbase : NULL;
}
int rm_native_flush(void) { return fflush(NULL); }
/* Registered before modules: their atexit callbacks run first; ELF fini runs
 * after this close, matching the public stdout lifetime of the reference. */
static atomic_int native_stdout_closed;
static void close_native_stdout(void) {
    int previous_error = ferror(stdout);
    atomic_store_explicit(&native_stdout_closed, 1, memory_order_release);
    int closed = fclose(stdout);
    int saved = errno;
    if (previous_error || closed != 0) {
        char message[1024];
        if (closed != 0)
            snprintf(message, sizeof message, "write error: stdout: %s", strerror(saved));
        else
            snprintf(message, sizeof message, "write error: stdout");
        if (active) {
            active->request.text = message;
            yield_request(active, 5);
        } else {
            extern char *program_invocation_short_name;
            fprintf(stderr, "%s: %s\n", program_invocation_short_name, message);
        }
        exit(2);
    }
}
static pthread_once_t stdio_once = PTHREAD_ONCE_INIT;
static void initialize_stdio_once(void) {
    atexit(close_native_stdout);
    setvbuf(stdout, NULL, _IOLBF, BUFSIZ);
}
void rm_native_stdio_initialize(void) {
    pthread_once(&stdio_once, initialize_stdio_once);
}
int rm_native_flush_stream(int error) { return fflush(error ? stderr : stdout); }

int rm_native_stdout_closed(void) {
    return atomic_load_explicit(&native_stdout_closed, memory_order_acquire);
}

/* Remains valid during libc finalization, after Rust's TLS destructors. */
uintptr_t rm_native_thread_id(void) { return (uintptr_t)pthread_self(); }

/* Diagnostic conversion reads the current pthread locale and rounding mode.
 * Integer operands avoid an earlier Rust conversion changing zero's sign under
 * FE_DOWNWARD. One snprintf call formats each value; the usual path uses the
 * caller's stack buffer. No interpreter/descriptor/filesystem action occurs. */
int rm_database_percent(char *scratch, size_t scratch_capacity, char **owned,
                        uint64_t numerator, uint64_t denominator,
                        unsigned int fractional_digits) {
    int saved = errno;
    *owned = NULL;
    if (fractional_digits > 1) { errno = EINVAL; return -1; }
    const struct lconv *numeric = localeconv();
    size_t decimal = strlen(numeric->decimal_point);
    if (decimal > SIZE_MAX - 64) { errno = ENOMEM; return -1; }
    size_t capacity = decimal + 64;
    char *buffer = scratch;
    if (capacity > scratch_capacity) {
        buffer = malloc(capacity);
        if (!buffer) return -1;
        *owned = buffer;
    } else {
        capacity = scratch_capacity;
    }
    double value = numerator && denominator
        ? (100.0 * (double)numerator) / (double)denominator : 0.0;
    int length = snprintf(buffer, capacity,
                          fractional_digits ? "%.1f" : "%.0f", value);
    if (length < 0 || (size_t)length >= capacity) {
        int failure = length < 0 ? errno : EOVERFLOW;
        free(*owned); *owned = NULL;
        errno = failure;
        return -1;
    }
    errno = saved;
    return length;
}
void rm_database_percent_free(char *owned) { free(owned); }
