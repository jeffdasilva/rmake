.SECONDEXPANSION:
SET = $(file >>seen,search)$(shell /bin/sh -c 'exit 7')
all: item.out
all:;@:
item.out: $$(info explicit=[$$@|$$(.SHELLSTATUS)])$$(file >>seen,explicit)
%.out: %.in $$(SET)
	@$(info recipe=[$@|$(.SHELLSTATUS)])$(file >>seen,recipe):
