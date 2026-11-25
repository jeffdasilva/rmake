ifeq ($(wildcard initialized),)
$(shell touch -d @1680300000 b.mk && \
  touch -d @1680299970 b.seed && \
  touch -d @1680300020 newer)
$(file >initialized,yes)
endif
stage := $(or $(MAKE_RESTARTS),0)
$(file >>proof,read=$(stage))
-include b.mk
b.mk:: newer
	+@touch -d @1680300040 b.mk && printf 'declared=%s\n' '$(stage)' >> proof
%.mk: %.seed
	@printf 'synthetic=%s\n' '$(stage)' >> proof
