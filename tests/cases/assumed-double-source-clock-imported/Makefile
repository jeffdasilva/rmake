ifeq ($(wildcard initialized),)
$(shell touch -d @1680300000 b.mk stable)
$(file >initialized,yes)
endif
stage := $(or $(MAKE_RESTARTS),0)
$(file >>proof,read=$(stage))
-include b.mk
b.mk:: | stable
	@false
%.mk:
	@printf 'synthetic=%s\n' '$(stage)' >> proof
all:
	@printf 'goal=%s\n' '$(stage)' >> proof
	@cat proof
	@stat -c 'b.mk=%Y' b.mk
