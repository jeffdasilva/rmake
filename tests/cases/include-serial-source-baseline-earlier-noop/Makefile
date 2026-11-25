ifeq ($(wildcard initialized),)
$(shell touch -d @1680300000 b.mk clean.inc && \
  touch -d @1680299980 stable && touch -d @1680300020 newer)
$(file >initialized,yes)
endif
stage := $(or $(MAKE_RESTARTS),0)
source_time = $(if $(wildcard b.mk),$(shell stat -c %Y b.mk),absent)
$(file >>proof,read=$(stage) time=$(source_time))
include clean.inc
-include b.mk
$(file >>proof,read-payload=$(payload))
ifeq ($(MAKE_RESTARTS),)
clean.inc: newer
	@printf 'clean=%s time=%s\n' '$(stage)' '$(source_time)' >> proof && :
else
clean.inc: | stable
	@false
endif
b.mk: newer
	@printf 'remake=%s time=%s\n' '$(stage)' '$(source_time)' >> proof
all:
	@printf 'goal=%s time=%s payload=%s\n' '$(stage)' '$(source_time)' '$(payload)' >> proof
	@cat proof
