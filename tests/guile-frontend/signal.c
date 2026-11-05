/* Original finite SIGUSR1 debug witness; no GNU headers. */
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
extern void gmk_add_function(const char *, char *(*)(const char *, unsigned int, char **), unsigned int, unsigned int, unsigned int);
int plugin_is_GPL_compatible = 1;
static char *toggle(const char *name, unsigned int argc, char **argv) {
  (void)name; const char *label = argc ? argv[0] : "empty";
  FILE *file = fopen("seen", "a"); if (!file) _Exit(90);
  fprintf(file, "%s|before\n", label); if (fclose(file)) _Exit(91);
  if (raise(SIGUSR1)) _Exit(92);
  file = fopen("seen", "a"); if (!file) _Exit(93);
  fprintf(file, "%s|after\n", label); if (fclose(file)) _Exit(94);
  return NULL;
}
int signal_probe_gmk_setup(const void *location) {
  (void)location; gmk_add_function("phase_signal", toggle, 0, 1, 0); return 1;
}
