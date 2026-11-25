ifeq ($(wildcard initialized),)
$(shell touch -d @1680300000 b.mk stable broken.inc)
$(file >initialized,yes)
endif
stage := $(or $(MAKE_RESTARTS),0)
$(file >>proof,read=$(stage) mtime=$(shell TZ=UTC stat -c %y b.mk))
include b.mk
-include broken.inc
b.mk:: | stable
	@false
%.mk:
	+@$(file >>proof,forced-expansion):
	$(if $(wildcard second-entry),@printf 'blocked\n' >> proof,$(file >second-entry,yes))

broken.inc:: | stable
	@false
