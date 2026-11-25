ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1600000000 b.mk && touch -d @1599999999 old && touch -d @1600000001 seed)
endif
include b.mk
all: ; @echo all
b.mk:: old ; @echo head
b.mk:: seed ; $(eval MAKEFLAGS += -q)@echo tail
