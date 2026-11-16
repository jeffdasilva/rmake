.ONESHELL:
$(file >events,read)
BEFORE := $(shell /bin/sh -c 'exit 9')
empty :=
space := $(empty) $(empty)
LEXICAL := $$(LOCAL_EFFECT)
LOCAL_EFFECT = $(file >>events,local-flags)$(MISSING_LOCAL)-c
$(file >>events,before[$(.SHELLSTATUS)])
SHELL = $(file >>events,shell[$(.SHELLSTATUS)])$(MISSING_SHELL)/bin/sh
.SHELLFLAGS = $(file >>events,global-flags[$(.SHELLSTATUS)])$(MISSING_GLOBAL)-c
IFS = $(file >>events,ifs[$(.SHELLSTATUS)])$(MISSING_IFS)$(space)
export OBSERVE = $(file >>events,export[$(.SHELLSTATUS)])yes
CALL_CAPTURE = $(let .SHELLFLAGS,$1,$(shell $2))$(file >>events,after-call[$(.SHELLSTATUS)])
all: ; @$(info result=[$(let .SHELLFLAGS,-ec,$(let .SHELLFLAGS,$(LEXICAL),$(shell printf captured; exit 6)$(file >>events,inner[$(.SHELLSTATUS)]))$(file >>events,outer[$(.SHELLSTATUS)]))])$(file >>events,after[$(.SHELLSTATUS)]$(MISSING_AFTER))$(eval SHELL := /bin/sh)$(eval .SHELLFLAGS := -c)$(eval IFS :=):
