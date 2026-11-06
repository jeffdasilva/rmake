.SECONDEXPANSION:
CHECK = $(eval all: RESULT := $$(info active=[$$(V)|$$@|$$<|$$(.SHELLSTATUS)])$$(file >>seen,active))
all: V := selected
all: stamp
all: $$(shell /bin/sh -c 'exit 7') $$(CHECK)
all:;@:
stamp:;@:
