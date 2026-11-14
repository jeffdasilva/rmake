empty :=
space := $(empty) $(empty)
tab := $(empty)	$(empty)
define lf


endef
bs := $(subst marker,\,marker)
BEFORE := $(shell /bin/sh -c 'exit 9')
SHELL := ./shell-helper
.SHELLFLAGS := -q
export WATCH = $(file >>exports,ran)yes
VALUE != $(tab)$(tab)first
$(info shell=[$(VALUE)] status=[$(.SHELLSTATUS)])
$(info exports=[$(file <exports)] flags=[$(file <flags)])
all:
