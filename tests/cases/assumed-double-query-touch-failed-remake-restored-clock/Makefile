ifeq ($(wildcard initialized),)
$(shell touch -d @1680300000 b.mk stable broken.inc)
$(file >initialized,yes)
endif
stage := $(or $(MAKE_RESTARTS),0)
$(file >>proof,read=$(stage) mtime=$(shell if test b.mk -nt stable; then printf touched; else printf original; fi))
-include b.mk broken.inc
b.mk:: | stable
	@false
%.mk: ;
broken.inc:: restore-clock
	@false
restore-clock:
	@if test b.mk -nt stable; then printf 'restore after touch\n' >> proof; fi; touch -r stable b.mk; false
check-clock: ; +@test b.mk -nt stable && printf 'physical=touched\n' >> proof
