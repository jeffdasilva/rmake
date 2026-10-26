.DEFAULT_GOAL := out
out peer &: $$(X)
ifeq ($(eval .SECONDEXPANSION:),)
endif
	@printf '[%s|%s|%s]\n' '$@' '$^' '$|'
X=late.in
late.in:;@:
