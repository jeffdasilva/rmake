include settings.mk
all: ; @printf 'restart=%s,value=%s\n' '$(MAKE_RESTARTS)' '$(VALUE)'
.SECONDEXPANSION:
settings.mk:
probe = $(if $(MAKE_RESTARTS),,$(file >>settings.mk,VALUE += changed))$(file >>seen,phase=$(MAKE_RESTARTS))
%.mk: $$(probe)absent.src
	@:
