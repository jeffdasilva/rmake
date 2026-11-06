.SECONDEXPANSION:
SET = $(file >>seen,search)
all: item.out $$(shell /bin/sh -c 'exit 3')
all:;@:
%.out: %.in $$(SET)
	@$(info recipe=[$@|$(.SHELLSTATUS)])$(file >>seen,recipe):
