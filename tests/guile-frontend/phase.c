/* Original finite public phase witness; no GNU headers or implementation. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <fcntl.h>
#include <locale.h>
#include <pthread.h>
#include <signal.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <stdio_ext.h>
#include <string.h>
#include <sys/syscall.h>
#include <unistd.h>
#include <wchar.h>
extern void gmk_add_function(const char *, char *(*)(const char *, unsigned int, char **), unsigned int, unsigned int, unsigned int);
int plugin_is_GPL_compatible = 1;
static pthread_t original_thread;
static char handler_kind(int number) {
  struct sigaction action;
  if (sigaction(number, NULL, &action)) return '?';
  return action.sa_handler == SIG_DFL ? 'D' : action.sa_handler == SIG_IGN ? 'I' : 'H';
}
static void observe(const char *label) {
  int initialized = -1;
  void *pointer = dlsym(RTLD_DEFAULT, "scm_initialized_p");
  if (pointer) memcpy(&initialized, pointer, sizeof initialized);
  pthread_attr_t attributes;
  void *stack = NULL; size_t size = 0; char local;
  int on_stack = 0;
  if (!pthread_getattr_np(pthread_self(), &attributes)) {
    if (!pthread_attr_getstack(&attributes, &stack, &size)) {
      uintptr_t location = (uintptr_t)&local, base = (uintptr_t)stack;
      on_stack = location >= base && location - base < size;
    }
    pthread_attr_destroy(&attributes);
  }
  int final = !strcmp(label, "fini");
  int stdout_open = fcntl(STDOUT_FILENO, F_GETFD) >= 0;
  int orientation = final ? 0 : fwide(stdout, 0);
  int line_buffered = final ? 0 : !!__flbf(stdout);
  FILE *file = fopen("seen", "a");
  if (!file) _Exit(90);
  fprintf(file, "%s|init=%d|leading=%d|same-thread=%d|ordinary-stack=%d|locale=%s|stdout-open=%d", label,
          initialized != 0 ? initialized : 0, (int)(syscall(SYS_gettid) == getpid()),
          (int)pthread_equal(pthread_self(), original_thread), on_stack,
          setlocale(LC_ALL, NULL), stdout_open);
  if (!final) fprintf(file, "|orientation=%d|line=%d", orientation, line_buffered);
  fprintf(file, "|int=%c|usr1=%c\n", handler_kind(SIGINT), handler_kind(SIGUSR1));
  if (fclose(file)) _Exit(91);
}
__attribute__((constructor)) static void before(void) {
  original_thread = pthread_self(); observe("ctor");
#ifdef PHASE_REDIRECT
  if (!freopen("/dev/null", "w", stdout)) _Exit(92);
  setvbuf(stdout, NULL, _IOLBF, 0);
#endif
}
__attribute__((destructor)) static void after(void) { observe("fini"); }
static char *phase(const char *name, unsigned int argc, char **argv) {
  (void)name; (void)argc; (void)argv; observe("probe"); return NULL;
}
static int setup(void) { gmk_add_function("phase", phase, 0, 0, 0); return 1; }
int phase_probe_gmk_setup(const void *location) { (void)location; return setup(); }
int phase_redirect_gmk_setup(const void *location) { (void)location; return setup(); }
