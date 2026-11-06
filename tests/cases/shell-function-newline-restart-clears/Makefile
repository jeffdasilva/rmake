include turn.mk
$(info pass=[$(MAKE_RESTARTS)] result=[$(shell $(file <raw))])
ifeq ($(MAKE_RESTARTS),)
.PHONY: reset-oneshell
turn.mk: reset-oneshell ; @: > $@
endif
all: ; @:
