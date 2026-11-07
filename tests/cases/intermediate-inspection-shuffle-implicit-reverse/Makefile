$(file >seen,anchor)
.PHONY: all one two three
all: t.bundle
%.bundle: %.middle
	@cat $< > $@
%.middle: one two three
	@printf middle > $@
one two three: ; @$(info $@)$(file >>seen,$@):
