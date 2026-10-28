override CC=user
override CFLAGS=user
override FC=user
override FFLAGS=user
override SCCSGETFLAGS=user
override ARFLAGS=user
.POSIX:
$(foreach n,.SHELLFLAGS CC CFLAGS FC FFLAGS SCCSGETFLAGS ARFLAGS OUTPUT_OPTION,$(info $(n)=[$($(n))]/[$(flavor $(n))]/[$(origin $(n))]))
all: ; @true
