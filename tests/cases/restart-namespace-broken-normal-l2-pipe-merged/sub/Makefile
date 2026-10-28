include config.mk
WARM:=$(wildcard old*.in)
ifeq ($(MAKE_RESTARTS),)
MOVE:=$(shell python3 move.py broken Makefile)
endif
$(info phase=old:$(notdir $(CURDIR)):$(MAKE_RESTARTS):$(MARKER))
include generated.mk
all: old.out
	@$(file >where,$(abspath .))printf '%s\n' OLD:$(MARKER)
%.out: %.in
	@cp $< $@
generated.mk: ; @printf '%s\n' READY=yes > $@
