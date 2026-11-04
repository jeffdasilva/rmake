/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright 2026 Jeff DaSilva
 * Independent observation of public setup location pointer and line. */
#include <stdio.h>
#include <stdlib.h>
struct Place { const char *file; unsigned long line; };
int plugin_is_GPL_compatible=1;
static int setup(const struct Place *place){
    FILE *f=fopen("seen","a");if(!f)exit(90);
    const char *tag=place->file?(place->file[0]?place->file:"EMPTY"):"NULL";
    fprintf(f,"SETUP:%s:%lu\n",tag,place->line);if(fclose(f))exit(91);return 1;
}
#ifdef PROV_RELOAD
int prov_reload_gmk_setup(const struct Place *place){return setup(place);}
__attribute__((destructor))static void finish(void){FILE*f=fopen("seen","a");if(!f)exit(90);fputs("DROP\n",f);if(fclose(f))exit(91);}
#else
int prov_probe_gmk_setup(const struct Place *place){return setup(place);}
#endif
