
$(info initial-goals=[$(MAKECMDGOALS)])
VPATH = src
GPATH = src
src/parent: DESTINATION_ONLY := destination
parent: input
	@printf 'build=[%s|%s]\n' '$@' '$<'; touch -r input src/parent
after:;@$(info late-goals=[$(MAKECMDGOALS)]):
hide:; @rm src/parent
