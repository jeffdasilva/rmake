.SECONDEXPANSION:
all: $$(shell /bin/sh -c 'exit 7')
all: $$(info next=[$$@|$$(.SHELLSTATUS)])$$(file >>seen,next)
all:;@$(info recipe=[$@|$(.SHELLSTATUS)]):
