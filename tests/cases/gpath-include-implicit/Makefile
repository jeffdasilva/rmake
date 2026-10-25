seed := $(shell touch -d @1600000000 src/config.mk src/config.in; touch -d @1600000002 src/config.in 2>/dev/null)
VPATH=src
GPATH=src
include config.mk
all:;@echo value=$(VALUE) restarts=$(MAKE_RESTARTS)
%.mk: %.in
	@echo rebuilding=$@; cp -p $< $@
