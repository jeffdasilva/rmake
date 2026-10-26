.DEFAULT_GOAL := out
$(file >gone.in,gone)
out: %: | *.in
ifeq ($(shell rm gone.in),)
endif
	@printf '[%s|%s|%s]\n' '$@' '$^' '$|'
*.in:;@:
