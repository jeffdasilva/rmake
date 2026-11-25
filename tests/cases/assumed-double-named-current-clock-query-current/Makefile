ifeq ($(wildcard initialized),)
$(shell touch -d @1680300000 b.mk stable && \
  touch -d @1680300020 b.seed)
$(file >initialized,yes)
endif
stage := $(or $(MAKE_RESTARTS),0)
$(file >>proof,read=$(stage))
include b.mk
b.mk:: | stable
	@false
%.mk: | %.seed
	@printf 'synthetic=%s\n' '$(stage)' >> proof
