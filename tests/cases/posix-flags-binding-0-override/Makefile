override .SHELLFLAGS=-c
$(info metadata=[$(value .SHELLFLAGS)]/[$(origin .SHELLFLAGS)]/[$(flavor .SHELLFLAGS)])
$(info shell=[$(shell false; printf reached)]/[$(.SHELLSTATUS)])
all: ; @false; printf reached\n
