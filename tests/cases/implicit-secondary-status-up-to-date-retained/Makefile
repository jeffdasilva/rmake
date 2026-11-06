.SECONDEXPANSION:
SET = $(file >>seen,search)$(shell /bin/sh -c 'exit 7')
all: prepare item.out after
all:;@:
.PHONY: prepare
prepare:
	@touch -d @946684800 item.in && touch -d @946684900 item.out
%.out: %.in $$(SET)
	@$(error unexpected recipe)
after:;@$(eval item.out: RESULT := $$(info retained=[$$(.SHELLSTATUS)])$$(file >>seen,retained)):
