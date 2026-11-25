ifeq ($(wildcard initialized),)
$(shell touch -d @1680300000 b.mk stable broken.mk && \
  touch -d @1680299970 b.seed)
$(file >initialized,yes)
endif
stage := $(or $(MAKE_RESTARTS),0)
$(file >>proof,read=$(stage))
-include broken.mk b.mk
broken.mk:: missing
	@false
b.mk:: | stable
	@false
%.mk: %.seed
	@printf 'synthetic=%s\n' '$(stage)' >> proof
