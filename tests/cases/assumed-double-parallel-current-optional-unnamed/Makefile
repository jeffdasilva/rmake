ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1680300000 b.mk stable)
$(shell touch -d @1680299990 b.seed)
endif
stage := $(or $(MAKE_RESTARTS),0)
$(info read=$(stage) clock=$(shell stat -c %Y b.mk))
-include b.mk
b.mk:: | stable ; @false
%.mk: b.seed ;
all: ; @printf "goal-%s\n" "$(stage)"
