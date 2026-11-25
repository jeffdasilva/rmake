ifeq ($(wildcard initialized),)
$(shell touch -d @1680300000 b.mk stable broken.inc)
$(file >initialized,yes)
endif
stage := $(or $(MAKE_RESTARTS),0)
$(file >>proof,read=$(stage) mtime=$(shell if test b.mk -nt stable; then printf touched; else printf original; fi))
-include broken.inc b.mk
b.mk:: | stable
	@false
%.mk: ; @printf 'ordinary must stay suppressed\n' >> proof
broken.inc:: missing
	@false
check-clock: ; +@test b.mk -nt stable && printf 'physical=touched\n' >> proof
