$(file >seen,anchor)
.SECONDEXPANSION:
.PHONY: all first other
all: first $$(eval MAKEFLAGS += --shuffle=2)
	@$(file >>seen,all)$(info all=[$(filter --shuffle=%,$(MAKEFLAGS))]):
first:;@$(file >>seen,first)$(info first=[$(filter --shuffle=%,$(MAKEFLAGS))]):
other:;@$(file >>seen,other)$(info other=[$(filter --shuffle=%,$(MAKEFLAGS))]):
