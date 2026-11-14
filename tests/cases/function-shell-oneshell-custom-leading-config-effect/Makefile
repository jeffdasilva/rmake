empty :=
space := $(empty) $(empty)
tab := $(empty)	$(empty)
define lf


endef
bs := $(subst marker,\,marker)
BEFORE := $(shell /bin/sh -c 'exit 9')
.ONESHELL:
SHELL := ./shell-helper
.SHELLFLAGS = $(file >>flags,configured)-q
export WATCH = $(file >>exports,ran)yes
$(info shell=[$(shell $(tab)$(bs)$(lf)$(tab))] status=[$(.SHELLSTATUS)])
$(info exports=[$(file <exports)] flags=[$(file <flags)])
all:
