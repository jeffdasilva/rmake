empty :=
space := $(empty) $(empty)
define lf


endef
bs := $(subst marker,\,marker)
BEFORE := $(shell /bin/sh -c 'exit 9')
SHELL := ./shell-helper
.SHELLFLAGS = $(file >>flags,configured)-q
export WATCH = $(file >>exports,ran)yes
$(info shell=[$(shell $(lf))] status=[$(.SHELLSTATUS)])
$(info exports=[$(file <exports)] flags=[$(file <flags)])
all:
