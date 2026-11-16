SHELL := /bin/sh
$(file >events,started)
export Y = $(file >>events,y-export)value
OTHER = $(file >>events,other[$(1)])$(if $(1),$(shell printf '[%s|%s]' "$${CHECK-absent}" "$${Y-absent}"),other-fallback)
CHECK = $(file >>events,body[$(1)])$(1)
export CHECK
all: ; @$(info [$(call CHECK,outer) $(shell printf '[%s|%s]' "$${CHECK-absent}" "$${Y-absent}")])$(file >>events,after):
