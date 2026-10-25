VPATH = src
all: foo
	@echo all=$^ first=$<
foo:: input
	@echo source=$@
src/foo:: input
	@echo first=$@
src/foo:: input
	@echo second=$@
