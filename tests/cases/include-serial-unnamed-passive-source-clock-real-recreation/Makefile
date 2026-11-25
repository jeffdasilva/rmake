ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1680300000 clean.inc b.mk result.1; touch -d @1680299990 stable; touch -d @1680300010 newer)
endif
stage := $(or $(MAKE_RESTARTS),0)
clock = $(if $(wildcard b.mk),$(shell stat -c %Y b.mk),absent)
$(info read=$(stage) clock=$(clock))
include clean.inc
-include b.mk
$(info payload=$(stage):$(payload))
.DEFAULT_GOAL := result.$(stage)
ifeq ($(MAKE_RESTARTS),)
clean.inc: newer
	@printf "earlier:%s:%s\n" "$(stage)" "$(clock)" >> proof; rm -f -- b.mk
b.mk:: newer
	@printf "branch-0:%s:%s\n" "$(stage)" "$(clock)" >> proof; :
b.mk:: newer
	@printf "branch-1:%s:%s\n" "$(stage)" "$(clock)" >> proof; printf "payload := new\n" > b.mk; touch -d @1680300020 b.mk
endif
result.0 result.1: ; @printf "recipe\n" > $@
