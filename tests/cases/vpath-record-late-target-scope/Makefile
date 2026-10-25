VPATH=src
all: setup foo
setup: ; @: $(eval src/foo: VALUE=late)
src/foo: ; @echo actual=$@:$(VALUE)
