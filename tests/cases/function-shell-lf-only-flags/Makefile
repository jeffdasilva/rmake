empty :=
space := $(empty) $(empty)
define lf


endef
bs := $(subst marker,\,marker)
BEFORE := $(shell /bin/sh -c 'exit 9')
.SHELLFLAGS = $(file >>flags,configured)-c
export WATCH = $(file >>exports,ran)yes
$(info shell=[$(shell $(lf))] status=[$(.SHELLSTATUS)])
$(info exports=[$(file <exports)] flags=[$(file <flags)])
all: ; @:
