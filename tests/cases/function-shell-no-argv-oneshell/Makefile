empty :=
space := $(empty) $(empty)
define lf


endef
bs := $(subst marker,\,marker)
BEFORE := $(shell /bin/sh -c 'exit 9')
.ONESHELL:
export WATCH = $(file >>exports,ran)yes
all: ; @$(info shell=[$(shell $(bs)$(lf)$(space))] status=[$(.SHELLSTATUS)])$(info exports=[$(file <exports)]):
