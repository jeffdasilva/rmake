/* Original differential fixture; public libc interfaces, no GNU headers. */
#define _GNU_SOURCE
#include <locale.h>
#include <stdlib.h>
#include <string.h>
struct location { const char *file; unsigned long line; };
typedef char *(*callback)(const char *, unsigned int, char **);
extern void gmk_add_function(const char *, callback, unsigned int, unsigned int, unsigned int);
extern char *gmk_alloc(unsigned int);
int plugin_is_GPL_compatible;
static locale_t owned;
static char *copy(const char *text) {
  if (!text) text="unavailable";
  size_t length=strlen(text);char *out=gmk_alloc((unsigned int)length+1);
  memcpy(out,text,length+1);return out;
}
static char *select_locale(const char *name,unsigned int argc,char **argv) {
  (void)name;(void)argc;
  return copy(setlocale(LC_COLLATE,argv[0]));
}
static char *select_thread(const char *name,unsigned int argc,char **argv) {
  (void)name;(void)argc;
  if (!strcmp(argv[0],"global")) {
    uselocale(LC_GLOBAL_LOCALE);
    if (owned) { freelocale(owned);owned=(locale_t)0; }
    return copy("global");
  }
  locale_t next=newlocale(LC_COLLATE_MASK,argv[0],(locale_t)0);
  if (!next) return copy("unavailable");
  uselocale(next);
  if (owned) freelocale(owned);
  owned=next;return copy("thread");
}
int locale_switch_gmk_setup(const struct location *location) {
  (void)location;
  gmk_add_function("lc_select",select_locale,1,1,0);
  gmk_add_function("lc_thread",select_thread,1,1,0);
  return 1;
}
