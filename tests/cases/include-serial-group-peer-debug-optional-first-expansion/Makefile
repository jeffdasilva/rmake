$(info READ:$(MAKE_RESTARTS))
.PHONY: all force
all: ; @echo ALL:$(VALUE)
-include payload.mk
include worker.mk
ifeq ($(MAKE_RESTARTS),)
force:
worker.mk payload.mk &: force ; @$(shell printf 'VALUE = new\n' > payload.mk) :
endif
