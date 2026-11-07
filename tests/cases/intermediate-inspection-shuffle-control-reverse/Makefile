$(file >seen,anchor)
.PHONY: all one two three
.NOTINTERMEDIATE: middle
all: bundle
bundle: middle
	@cat $< > $@
middle: one two three
	@printf middle > $@
one two three: ; @$(info $@)$(file >>seen,$@):
