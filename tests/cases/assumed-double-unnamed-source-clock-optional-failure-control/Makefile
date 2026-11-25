ifeq ($(wildcard initialized),)
$(shell touch -d @1680300000 b.mk stable broken.inc && touch -d @1680299970 b.seed && touch -d @1680300045 all && touch -d @1680300020 goal.seed)
$(file >initialized,yes)
endif
stage := $(or $(MAKE_RESTARTS),0)
$(file >>proof,read=$(stage) clock=$(shell TZ=UTC stat -c %y b.mk))
-include b.mk broken.inc
b.mk:: | stable
	@false
%.mk: b.seed ; @printf 'source=%s\n' '$(stage)' >> proof
broken.inc:: missing
	@false
all: b.mk goal.seed ; @printf 'caller=%s\n' '$(stage)' >> proof
