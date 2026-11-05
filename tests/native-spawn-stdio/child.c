/* SPDX-License-Identifier: GPL-3.0-or-later */
/* Copyright 2026 Jeff DaSilva */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

int main(int argc, char **argv) {
    if (argc != 2)
        return 97;
    const char *mode = argv[1];
    FILE *seen = fopen("seen", "a");
    if (!seen)
        return 98;
    if (!strcmp(mode, "private")) {
        struct stat entry;
        if (stat("private-stream", &entry) || printf("size=%ld\n", (long)entry.st_size) < 0
            || fprintf(seen, "child-size=%ld\n", (long)entry.st_size) < 0)
            return 99;
    } else if (!strcmp(mode, "public") || !strcmp(mode, "export")) {
        if (!strcmp(mode, "export") && (!getenv("MARK") || strcmp(getenv("MARK"), "visible")))
            return 100;
        if (fputs("child-out\n", stdout) == EOF || fputs("child-err\n", stderr) == EOF
            || fprintf(seen, "child-%s\n", mode) < 0)
            return 101;
    } else if (strcmp(mode, "effect") || fputs("child-effect\n", seen) == EOF) {
        return 102;
    }
    return fclose(seen) ? 103 : 0;
}
