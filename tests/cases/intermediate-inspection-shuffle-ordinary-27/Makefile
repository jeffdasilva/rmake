$(file >seen,anchor)
.PHONY: all one two three
all: bundle
bundle: middle
	@cat $< > $@
middle: one two three
	@printf middle > $@
one two three: ; @$(info $@)$(file >>seen,$@):
