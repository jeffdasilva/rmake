$(info goals=[$(MAKECMDGOALS)])
VPATH = src
GPATH = src
parent:: input
	@$(info branch=[$@|$<])touch -r input src/parent
parent:: other
	@$(info branch=[$@|$<]):
after last:
	@$(info marker=[$@]):
unused: src/parent
