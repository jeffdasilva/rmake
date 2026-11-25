ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1680300000 b.mk stable)
$(shell touch -d @1680299990 b.seed)
$(shell touch -d @1680300000 all)
$(shell touch -d @1680300010 goal.seed.0)
$(shell touch -d @1680299990 goal.seed.1)
endif
stage := $(or $(MAKE_RESTARTS),0)
$(info read=$(stage) clock=$(shell stat -c %Y b.mk))
include b.mk
b.mk:: | stable ; @false
%.mk: b.seed ;
all: goal.seed.$(stage) ; @printf "recipe\n" > $@
