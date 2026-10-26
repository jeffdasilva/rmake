.DEFAULT_GOAL := a.out
%.out: $$(X)
ifeq ($(eval .SECONDEXPANSION:),)
endif
	@printf '[%s|%s|%s]\n' '$@' '$^' '$|'
X=late.in
late.in:;@:
