VPATH=./src:other
GPATH=./src
all: foo
foo: VALUE=source
foo: ; @echo original=$@:$(VALUE)
src/foo: VALUE=destination
src/foo: ; @echo unused=$@
