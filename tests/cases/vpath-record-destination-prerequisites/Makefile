VPATH = src
all: foo
	@echo all=$^ first=$<
foo: source-in | source-order
src/foo: destination-in | destination-order
	@echo target=$@ normal=$^ order=$|; cp -p input $@
source-in destination-in source-order destination-order: ; @echo dep=$@
