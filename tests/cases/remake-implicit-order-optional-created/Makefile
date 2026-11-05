-include settings.mk
all: ; @printf 'restart=%s,value=%s\n' '$(MAKE_RESTARTS)' '$(VALUE)'
.SECONDEXPANSION:
probe = $(file >settings.mk,VALUE := born)$(file >>seen,probe=$@)
%.mk: $$(probe)absent.src
	@:
