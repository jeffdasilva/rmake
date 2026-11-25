ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1680300000 b.mk stable)
$(shell touch -d @1680299990 b.seed)
$(shell touch -d @1680300000 result.1)
endif
stage := $(or $(MAKE_RESTARTS),0)
$(info read=$(stage) clock=$(shell stat -c %Y b.mk))
include b.mk
b.mk:: | stable ; @false
%.mk: b.seed ;
.DEFAULT_GOAL := result.$(stage)
result.0 result.1: ; @printf "recipe\n" > $@
