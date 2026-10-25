seed := $(shell touch -d @1600000000 src/config.mk input; touch -d @1600000002 input 2>/dev/null)
VPATH=src
GPATH=src
include config.mk
all:;@echo value=$(VALUE) restarts=$(MAKE_RESTARTS)
config.mk: input
	@echo rebuilding=$@; cp -p $< $@
