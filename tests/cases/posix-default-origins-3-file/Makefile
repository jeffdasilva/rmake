CC=user
CFLAGS=user
FC=user
FFLAGS=user
SCCSGETFLAGS=user
ARFLAGS=user
.POSIX:
$(foreach n,.SHELLFLAGS CC CFLAGS FC FFLAGS SCCSGETFLAGS ARFLAGS OUTPUT_OPTION,$(info $(n)=[$($(n))]/[$(flavor $(n))]/[$(origin $(n))]))
all: ; @true
