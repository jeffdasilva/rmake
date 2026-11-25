ifeq ($(wildcard initialized),)
$(shell touch -d @1680300000 b.mk stable broken.inc)
$(file >initialized,yes)
endif
stage := $(or $(MAKE_RESTARTS),0)
$(file >>proof,read=$(stage) mtime=$(shell TZ=UTC stat -c %y b.mk))
-include b.mk broken.inc
b.mk:: | stable
	@false
%.mk: ;
broken.inc:: | stable
	@false

check-clock: ; +@test b.mk -nt stable && printf 'physical=changed\n' >> proof
