ifeq ($(wildcard initialized),)
$(shell touch -d @1680300000 b.mk stable broken.inc && touch -d @1680300010 b.seed)
$(file >initialized,yes)
endif
stage := $(or $(MAKE_RESTARTS),0)
$(file >>proof,read=$(stage) clock=$(shell TZ=UTC stat -c %y b.mk))
-include b.mk
include broken.inc
b.mk:: | stable
	@false
%.mk: b.seed ;
broken.inc:: restore-clock
	@false
restore-clock:
	@if test b.mk -nt stable; then printf 'had-touch\n' >> proof; fi; touch -r stable b.mk; false
