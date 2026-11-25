
$(info initial-goals=[$(MAKECMDGOALS)])
VPATH = src
GPATH = src
src/parent: DESTINATION_ONLY := destination
parent: input
	@$(info build=[$@|$<])$(file >src/parent)$(shell touch -r input src/parent)
after:;@$(info late-goals=[$(MAKECMDGOALS)]):
hide:; @rm src/parent
