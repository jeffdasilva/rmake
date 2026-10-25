.SECONDEXPANSION:
VPATH = src
all: foo
	@echo all=$^ first=$<
foo: SOURCE=original
foo: $$(info source=$$@/$$</$$^) input
src/foo: DEST=actual
src/foo: $$(info destination=$$@/$$</$$^/$$(SOURCE)/$$(DEST)) input
	@echo build=$@:$^; cp -p $< $@
