$(foreach n,.SHELLFLAGS CC CFLAGS FC FFLAGS SCCSGETFLAGS ARFLAGS OUTPUT_OPTION,$(info $(n)=[$($(n))]/[$(flavor $(n))]/[$(origin $(n))]))
N:=.POSIX
$(N):
$(foreach n,.SHELLFLAGS CC CFLAGS FC FFLAGS SCCSGETFLAGS ARFLAGS OUTPUT_OPTION,$(info $(n)=[$($(n))]/[$(flavor $(n))]/[$(origin $(n))]))
all: ; @printf done\n
