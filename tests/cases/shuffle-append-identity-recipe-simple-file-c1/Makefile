$(file >seen,anchor)
V=--shuffle=2
.PHONY: all first other
unexport MAKEFLAGS
all: MAKEFLAGS := --shuffle=identity
all:first
	@$(eval MAKEFLAGS += $$(V))$(eval V=--shuffle=reverse)$(file >>seen,all)$(info all=[$(filter --shuffle=%,$(MAKEFLAGS))] @=[$@]):
first:;@$(file >>seen,first)$(info first=[$(filter --shuffle=%,$(MAKEFLAGS))] @=[$@]):
other:;@$(file >>seen,other)$(info other=[$(filter --shuffle=%,$(MAKEFLAGS))] @=[$@]):
