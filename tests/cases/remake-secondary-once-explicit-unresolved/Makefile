include settings.mk
all: ; @:
.SECONDEXPANSION:
probe = $(file >>seen,probe=$@)
settings.mk:
%.mk: $$(probe)absent.src
	@:
