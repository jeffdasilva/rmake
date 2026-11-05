include settings.mk
all: ; @:
.SECONDEXPANSION:
probe = $(file >>seen,probe=$@)
.PHONY: settings.mk
%.mk: $$(probe)absent.src
	@:
