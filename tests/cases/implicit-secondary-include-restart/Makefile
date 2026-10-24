.DEFAULT_GOAL:=all
.SECONDEXPANSION:
include config.mk
all: ; @echo value=$(VALUE) restart=$(MAKE_RESTARTS)
%.mk: $$(info include=$$* restart=$$(MAKE_RESTARTS))%.template
	@cp $< $@
