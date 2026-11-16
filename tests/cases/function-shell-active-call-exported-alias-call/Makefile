SHELL := /bin/sh
$(file >events,started)
PASS = $(call CHECK,$1)
export ALIAS = $(call CHECK,$(file >>events,alias-arg)export)
CHECK = global
all: CHECK = $(file >>events,body[$1])$(if $(filter outer,$1),$(shell printf "[%s]" "$${ALIAS-absent}"),inner)
all: ; @$(info result=$(call CHECK,outer))$(file >>events,after):
