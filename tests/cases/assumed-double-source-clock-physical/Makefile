ifeq ($(wildcard initialized),)
$(shell touch -d @1680300000 b.mk broken.mk && \
  touch -d @1680300020 b.seed && \
  touch -d @1680300020 seed)
$(file >initialized,yes)
endif
stage := $(or $(MAKE_RESTARTS),0)
$(file >>proof,read=$(stage))
-include broken.mk b.mk
broken.mk:: missing
	@false
b.mk:: seed
	@printf 'declared=%s\n' '$(stage)' >> proof && touch -d @1680300040 b.mk
%.mk: %.seed
	@printf 'synthetic=%s\n' '$(stage)' >> proof
all:
	@printf 'goal=%s\n' '$(stage)' >> proof
	@cat proof
	@stat -c 'b.mk=%Y' b.mk
