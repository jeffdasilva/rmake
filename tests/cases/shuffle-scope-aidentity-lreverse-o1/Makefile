$(file >seen,anchor)
.SECONDEXPANSION:
.PHONY: all first other
all: MAKEFLAGS = --shuffle=identity
all: first $$(eval MAKEFLAGS += --shuffle=reverse)
	@$(file >>seen,all)$(info all=[$(filter --shuffle=%,$(MAKEFLAGS))]):
first:;@$(file >>seen,first)$(info first=[$(filter --shuffle=%,$(MAKEFLAGS))]):
other:;@$(file >>seen,other)$(info other=[$(filter --shuffle=%,$(MAKEFLAGS))]):
