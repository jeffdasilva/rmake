SHELL := /bin/sh
$(file >events,started)
export Y = $(file >>events,y-export)value
OTHER = $(file >>events,other[$(1)])$(if $(1),$(shell printf '[%s|%s]' "$${CHECK-absent}" "$${Y-absent}"),other-fallback)
CHECK = $(file >>events,global)global
export ALIAS = $(file >>events,alias)$(CHECK)
all: CHECK = $(file >>events,body[$(1)])$(if $(1),$(let X,local,$(shell printf '[%s|%s]' "$${CHECK-absent}" "$${Y-absent}")),$(file >>events,inactive[$(1)])fallback)
all: ; @$(info [$(call CHECK,outer)])$(file >>events,after):
