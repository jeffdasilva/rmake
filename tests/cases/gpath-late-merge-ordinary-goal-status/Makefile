$(info goals=[$(MAKECMDGOALS)])
VPATH = src
GPATH = src
src/parent: DESTINATION_ONLY := destination
parent: input
	@$(info build=[$@|$<])touch -r input src/parent
after last:
	@$(info marker=[$@]):
