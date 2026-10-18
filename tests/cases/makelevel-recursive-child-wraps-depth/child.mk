all:
	@printf 'child-make=<%s> next=<%s>\n' '$(MAKELEVEL)' "$$MAKELEVEL"
