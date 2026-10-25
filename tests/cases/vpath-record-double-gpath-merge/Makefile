VPATH=src
GPATH=src
all: foo
foo:: input
foo:: never-built
src/foo:: ; @echo first=$@:$^
src/foo:: ; @echo second=$@:$^
