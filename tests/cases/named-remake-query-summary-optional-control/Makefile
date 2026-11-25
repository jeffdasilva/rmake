ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1600000000 b.mk && touch -d @1600000001 seed)
endif
-include b.mk
all: ; @echo all
b.mk: seed
	$(eval MAKEFLAGS += -qt)@echo remake
