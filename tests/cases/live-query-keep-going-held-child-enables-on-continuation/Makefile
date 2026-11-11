gate: ; @$(info gate):
cached:: ; @$(info head):
cached:: ; @$(info tail):
enable: SHELL = $(if $(wildcard ready),$(eval override MAKEFLAGS += -k)$(info toggled))/bin/sh
enable:
	+@: >ready; until test -f released; do sleep 0.01; done
	@:
release:
	+@until test -f ready; do sleep 0.01; done; printf 'release\n'; : >released
