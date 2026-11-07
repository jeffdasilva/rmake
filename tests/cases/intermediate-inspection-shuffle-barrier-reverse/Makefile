$(file >seen,anchor)
.PHONY: all one two three
.INTERMEDIATE: middle
all: bundle
bundle: middle
	@cat $< > $@
middle: one two .WAIT three
	@printf middle > $@
one two three: ; @$(info $@)$(file >>seen,$@):
