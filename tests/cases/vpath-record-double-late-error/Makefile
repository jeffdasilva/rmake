.SECONDEXPANSION:
VPATH=src
all: foo
foo:: input
foo:: $$(error source-tail-must-stay-unused)
src/foo:: ; @echo first=$@:$^
src/foo:: ; @echo second=$@:$^
