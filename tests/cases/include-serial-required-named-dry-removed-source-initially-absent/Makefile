ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1680300000 clean.inc; touch -d @1680299990 stable; touch -d @1680300010 newer)
endif
stage := $(or $(MAKE_RESTARTS),0)
source_time = $(if $(wildcard b.mk),$(shell stat -c %Y b.mk),absent)
$(info read=$(stage) time=$(source_time))
include clean.inc
include b.mk
$(info data=$(stage):$(payload))
ifeq ($(MAKE_RESTARTS),)
clean.inc: newer
	@printf "clean:%s:%s\n" "$(stage)" "$(source_time)" >> proof; rm -f -- b.mk
b.mk:: newer
	@printf "branch-0:%s:%s\n" "$(stage)" "$(source_time)" >> proof; :
b.mk:: newer
	@printf "branch-1:%s:%s\n" "$(stage)" "$(source_time)" >> proof; :
endif
all: ; @false
