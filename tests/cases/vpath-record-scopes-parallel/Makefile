VPATH = src
all: foo
	@echo all=$^ first=$<
foo: VALUE=source
foo: ONLY=source-only
src/foo: VALUE=destination
src/foo: input
	@echo value=$(VALUE) only=$(ONLY) target=$@; cp -p $< $@
