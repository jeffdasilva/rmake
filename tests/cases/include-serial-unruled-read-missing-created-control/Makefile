$(info READ:$(MAKE_RESTARTS))
.PHONY: all force
all: ; @echo ALL:$(VALUE)
include worker.mk
include payload.mk
ifeq ($(MAKE_RESTARTS),)
worker.mk: force ; @printf 'VALUE = new\n' > payload.mk; touch -t 200101010000 payload.mk
endif
