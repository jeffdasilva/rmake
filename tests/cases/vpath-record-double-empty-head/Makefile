VPATH=src
all: foo
foo:: source-one
	@echo source=$@:$^
foo:: never-built
src/foo:: dest-one
src/foo:: dest-two
	@echo second=$@:$^
source-one dest-one dest-two: ; @echo dep=$@
