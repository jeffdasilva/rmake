MAKELEVEL = 99
SNAP := <$(MAKELEVEL)> <$(value MAKELEVEL)> <$(origin MAKELEVEL)> <$(flavor MAKELEVEL)>
all:
	@printf '%s\n' '$(SNAP)' '<$(MAKELEVEL)> <$(value MAKELEVEL)> <$(origin MAKELEVEL)> <$(flavor MAKELEVEL)>'
	@printf 'child=<%s>\n' "$$MAKELEVEL"
