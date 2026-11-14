empty :=
space := $(empty) $(empty)
define lf


endef
bs := $(subst marker,\,marker)
BEFORE := $(shell /bin/sh -c 'exit 9')
SHELL := ./shell-helper
.SHELLFLAGS := -q
export WATCH = $(file >>exports,ran)yes
VALUE != $(space)$(lf)$(lf)
$(info shell=[$(VALUE)] status=[$(.SHELLSTATUS)])
$(info exports=[$(file <exports)] flags=[$(file <flags)])
all:
