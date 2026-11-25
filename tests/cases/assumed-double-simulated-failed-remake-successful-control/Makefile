ifeq ($(wildcard initialized),)
$(shell touch -d @1680300000 b.mk stable broken.inc)
$(file >initialized,yes)
endif
stage := $(or $(MAKE_RESTARTS),0)
$(file >>proof,read=$(stage) mtime=$(shell stat -c %Y b.mk))
-include broken.inc b.mk
broken.inc:: | stable
	@false
b.mk:: | stable
	@false
%.mk:
	@printf 'synthetic=%s\n' '$(stage)' >> proof
