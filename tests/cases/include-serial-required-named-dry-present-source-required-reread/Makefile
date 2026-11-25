ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1680300000 b.mk; touch -d @1680299990 steady; touch -d @1680300010 fresh)
endif
stage := $(or $(MAKE_RESTARTS),0)
ifneq ($(filter-out 0 1,$(stage)),)
$(error more than one reread)
endif
$(info read=$(stage) $(shell stat -c %Y b.mk))
include b.mk
$(info payload=$(stage):$(PAYLOAD))
.DEFAULT_GOAL := all
ifeq ($(MAKE_RESTARTS),)
b.mk:: fresh
	@printf "branch-0:%s\n" "$(stage)" >> proof; :
b.mk:: fresh
	@printf "branch-1:%s\n" "$(stage)" >> proof; :
endif
all: ; @false
