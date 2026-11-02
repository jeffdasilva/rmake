$(shell python3 setup.py)
all: configure dep
	@printf 'ALL=[%s]\n' '$?'; printf 'all-built\n' > all
dep: pre
	@printf 'DEP\n'; printf 'dep-built\n' > dep
pre:
	@printf 'PRE\n'; printf 'pre-built\n' > pre
.PHONY: configure
configure:
	@$(eval MAKEFLAGS += -Wdep):
