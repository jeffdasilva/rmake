$(foreach n,.SHELLFLAGS CC CFLAGS FC FFLAGS SCCSGETFLAGS ARFLAGS OUTPUT_OPTION,$(info $(n)=[$($(n))]/[$(flavor $(n))]/[$(origin $(n))]))
include mode.mk
$(foreach n,.SHELLFLAGS CC CFLAGS FC FFLAGS SCCSGETFLAGS ARFLAGS OUTPUT_OPTION,$(info $(n)=[$($(n))]/[$(flavor $(n))]/[$(origin $(n))]))
all: ; @printf done\n
