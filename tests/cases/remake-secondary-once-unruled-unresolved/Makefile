include settings.mk
all: ; @:
.SECONDEXPANSION:
probe = $(file >>seen,probe=$@)
%.mk: $$(probe)absent.src
	@:
