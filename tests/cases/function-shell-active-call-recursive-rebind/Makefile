SHELL := /bin/sh
$(file >events,started)
export Y = $(file >>events,y-export)value
OTHER = $(file >>events,other[$(1)])$(if $(1),$(shell printf '[%s|%s]' "$${CHECK-absent}" "$${Y-absent}"),other-fallback)
CHECK = $(file >>events,body[$(1)])$(if $(1),$(eval CHECK = $$(file >>events,new-body)changed)$(shell printf '[%s|%s]' "$${CHECK-absent}" "$${Y-absent}"),inactive)
export CHECK
$(info [$(call CHECK,outer)])$(file >>events,after)
all:
