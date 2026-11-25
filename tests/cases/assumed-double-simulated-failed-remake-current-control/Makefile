ifeq ($(wildcard initialized),)
$(shell touch -d @1680300000 b.mk stable broken.inc && \
  touch -d @1680299970 b.seed)
$(file >initialized,yes)
endif
stage := $(or $(MAKE_RESTARTS),0)
$(file >>proof,read=$(stage) mtime=$(shell stat -c %Y b.mk))
-include b.mk broken.inc
b.mk:: | stable
	@false
%.mk: %.seed
	@printf 'synthetic=%s\n' '$(stage)' >> proof
broken.inc:: missing
	@false
