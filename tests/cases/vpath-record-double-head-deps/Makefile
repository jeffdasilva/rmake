VPATH=src
all: foo
foo:: source-one
foo:: never-built
src/foo:: dest-one
	@echo first=$@:$^
src/foo:: dest-two
	@echo second=$@:$^
source-one dest-one dest-two: ; @echo dep=$@
