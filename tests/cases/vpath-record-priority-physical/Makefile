seed := $(shell touch -d @1600000000 src/foo; touch -d @1600000002 input)
VPATH = src:other
all: foo
	@echo all=$^
other/foo: ; @echo selected=$@
