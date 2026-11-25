ifeq ($(wildcard initialized),)
$(shell touch -d @1680299970 b.seed; touch -d @1680300045 all)
$(shell touch -d @1680300000 b.mk stable broken.inc)
$(shell touch -d @1680300010 b.seed)
$(file >initialized,yes)
endif
stage := $(or $(MAKE_RESTARTS),0)
$(file >>proof,read=$(stage) mtime=$(shell stat -c %Y b.mk) flags=$(firstword $(MAKEFLAGS)))
include b.mk
-include broken.inc
b.mk:: | stable
	@false
%.mk: b.seed
	+@:
	@printf 'ordinary recipe ran\n' >> proof
broken.inc:: | stable
	@false
