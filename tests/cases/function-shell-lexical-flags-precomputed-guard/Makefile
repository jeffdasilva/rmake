SHELL := /bin/sh
empty :=
start := $(shell /bin/sh -c 'exit 9')
$(file >events,started)
RAW = $(file >>events,flags[$(GUARD)])$(if $(GUARD),-c,$(let GUARD,leaf,$(shell /bin/sh -c 'exit 6')$(file >>events,leaf[$(.SHELLSTATUS)]))-c)
CALL = $(shell printf called)
LEXICAL := $$(if $$(GUARD),-c,$$(RAW))
.SHELLFLAGS = $(file >>events,global)-c
$(info result=[$(let .SHELLFLAGS,$(LEXICAL),$(shell printf result)$(file >>events,local[$(.SHELLSTATUS)]))])$(file >>events,after[$(.SHELLSTATUS)]$(MISSING_AFTER))
all:
