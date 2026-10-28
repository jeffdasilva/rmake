MAKEFLAGS += -C never-enter -f never-read.mk
$(info phase=[$(MAKE_RESTARTS)] flags=[$(MAKEFLAGS)] gnu=[$(GNUMAKEFLAGS)] files=[$(MAKEFILE_LIST)])
all: ; @$(info phase=[$(MAKE_RESTARTS)] flags=[$(MAKEFLAGS)] gnu=[$(GNUMAKEFLAGS)] files=[$(MAKEFILE_LIST)])true
include generated.mk
generated.mk: ; @printf 'READY=yes\n' > $@
