ifeq ($(wildcard initialized),)
$(shell touch -d @1680299970 b.seed; touch -d @1680300045 all)
$(shell touch -d @1680300000 b.mk stable broken.inc)
$(shell touch -r b.mk b.seed)
$(file >initialized,yes)
endif
stage := $(or $(MAKE_RESTARTS),0)
$(file >>proof,read=$(stage) mtime=$(shell stat -c %Y b.mk) flags=$(firstword $(MAKEFLAGS)))
-include b.mk broken.inc
b.mk:: | stable
	@false
.SECONDEXPANSION:
%.mk: b.seed $$(file >>proof,search=$$(stage)) ;
broken.inc:: | stable
	@false
