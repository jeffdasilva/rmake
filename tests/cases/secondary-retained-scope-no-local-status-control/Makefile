.SECONDEXPANSION:
BASE := $(shell /bin/sh -c 'exit 3')
all: $$(info during=[$$@|$$(.SHELLSTATUS)])$$(file >>seen,during)
all:;@$(info recipe=[$@|$(.SHELLSTATUS)]):
