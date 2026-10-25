VPATH=src
VALUE=global
all: foo
foo: VALUE+=source
src/foo: VALUE+=destination
src/foo: ; @echo actual=$@:$(VALUE)
