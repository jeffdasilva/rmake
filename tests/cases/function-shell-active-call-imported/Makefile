SHELL := /bin/sh
$(file >events,started)
export Y = $(file >>events,y-export)value
OTHER = $(file >>events,other[$(1)])$(if $(1),$(shell printf '[%s|%s]' "$${CHECK-absent}" "$${Y-absent}"),other-fallback)
CHECK = $(file >>events,body[$(1)])$(if $(1),$(shell printf '[%s|%s]' "$${CHECK-absent}" "$${Y-absent}"),$(file >>events,inactive[$(1)])fallback)
export CHECK
$(info [$(call CHECK,outer)])$(file >>events,after)
all:
