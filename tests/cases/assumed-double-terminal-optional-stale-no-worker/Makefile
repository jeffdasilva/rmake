ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1680300000 left.gen b.mk stable)
$(shell touch -d @1680299990 left.seed)
$(shell touch -d @1680300010 b.seed)
endif
stage := $(or $(MAKE_RESTARTS),0)
$(info read=$(stage) clock=$(shell stat -c %Y left.gen b.mk))
include left.gen
-include b.mk
left.gen: left.seed ; @false
b.mk:: | stable ; @false
%.mk: b.seed ;
