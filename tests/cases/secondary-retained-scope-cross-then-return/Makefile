.SECONDEXPANSION:
CHECK = $(eval first: RESULT := $$(eval after: BACK := $$$$(info caller=[$$$$@|$$$$(.SHELLSTATUS)])$$$$(file >>seen,caller))$$(info retained=[$$@|$$<|$$(.SHELLSTATUS)])$$(file >>seen,retained))
all: first after
.PHONY: first
first: stamp
first: $$(shell /bin/sh -c 'exit 7')
after: $$(shell /bin/sh -c 'exit 11') $$(CHECK)
after:;@$(info recipe=[$@|$(.SHELLSTATUS)]):
stamp:;@:
