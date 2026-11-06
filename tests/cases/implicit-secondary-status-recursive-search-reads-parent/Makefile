.SECONDEXPANSION:
SET = $(file >>seen,search)$(shell /bin/sh -c 'exit 7')
CHECK = $(eval item.out: RESULT := $$(info parent=[$$(.SHELLSTATUS)])$$(file >>seen,parent))
all: item.out
all:;@:
.PRECIOUS: %.mid
%.out: $$(SET) %.mid
	@$(info recipe=[$@|$(.SHELLSTATUS)])$(file >>seen,recipe):
%.mid: %.in $$(CHECK)
	@:
