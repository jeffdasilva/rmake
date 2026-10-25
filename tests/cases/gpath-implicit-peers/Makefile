seed := $(shell touch -d @1600000000 src/foo.out input src/foo.peer src/foo.in; touch -d @1600000002 input src/foo.in)
VPATH=src
GPATH=src
all: foo.out foo.peer
%.out %.peer &: %.in
	@echo owner=$@ all=$^; cp $< $*.out; cp $< $*.peer
