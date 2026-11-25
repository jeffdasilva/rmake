ifeq ($(wildcard initialized),)
$(shell touch -d @1680300000 b.mk clean.inc && \
  touch -d @1680299970 stable && touch -d @1680300020 newer)
$(file >initialized,yes)
endif
stage := $(or $(MAKE_RESTARTS),0)
source_time = $(shell stat -c %Y b.mk)
$(file >>proof,read=$(stage) time=$(source_time))
include clean.inc
include b.mk
$(file >>proof,read-payload=$(value))
ifeq ($(MAKE_RESTARTS),)
clean.inc: newer
	@printf 'clean=%s time=%s\n' '$(stage)' '$(source_time)' >> proof && touch -d @1680300010 b.mk
b.mk:: | stable
	@printf 'unexpected current recipe\n' >> proof
b.mk:: newer
	@printf 'branch-1=%s time=%s\n' '$(stage)' '$(source_time)' >> proof && printf 'value := branch-1\n' > b.mk && touch -d @1680300000 b.mk && false
else
clean.inc b.mk: | stable
	@false
endif
all:
	@printf 'goal=%s time=%s value=%s\n' '$(stage)' '$(source_time)' '$(value)' >> proof
	@cat proof
