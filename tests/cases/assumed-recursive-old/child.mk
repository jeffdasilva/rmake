all: dep
	@printf 'ALL=[%s]\n' '$?'; printf 'all-built\n' > all
dep: pre
	@printf 'DEP\n'; printf 'dep-built\n' > dep
pre:
	@printf 'PRE\n'; printf 'pre-built\n' > pre
