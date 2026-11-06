.SECONDEXPANSION:
SET = $(file >>seen,search)$(shell /bin/sh -c 'exit 7')
all: lib.a(item.o)
all:;@:
lib.a(%.o): $$(SET) %.missing
	@$(error unexpected recipe)
(%.o): %.in
	@$(info archive=[$@|$%|$(.SHELLSTATUS)])$(file >>seen,archive):
