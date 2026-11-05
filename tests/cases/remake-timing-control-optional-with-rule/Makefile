-include settings.mk
all: ; @:
.SECONDEXPANSION:
settings.mk:
probe = $(file >>seen,probe=$@)
%.mk: $$(probe)absent.src
	@:
