.SECONDEXPANSION:
all: first after
.PHONY: first
first: stamp
first: $$(shell /bin/sh -c 'exit 7')
after:;@$(eval first: RESULT := $$(info retained=[$$@|$$<|$$(.SHELLSTATUS)])$$(file >>seen,retained)):
stamp:;@:
