.SECONDEXPANSION:
ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1600000001 seed)
endif
include b.mk
all: ; @echo all
b.mk: $$(eval MAKEFLAGS += -q) seed
	@echo remake
