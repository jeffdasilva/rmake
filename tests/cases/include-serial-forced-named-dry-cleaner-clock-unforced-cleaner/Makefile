ifeq ($(MAKE_RESTARTS),)
$(shell touch -d @1680300000 b.mk clean.inc; touch -d @1680299990 steady; touch -d @1680300010 fresh)
endif
stage := $(or $(MAKE_RESTARTS),0)
ifneq ($(filter-out 0 1,$(stage)),)
$(error more than one reread)
endif
stamp = $(if $(wildcard b.mk),$(shell stat -c %Y b.mk),absent)
$(info READ:$(stage):$(stamp))
include clean.inc
include b.mk
$(info DATA:$(stage):$(payload))
.DEFAULT_GOAL := all
force-prefix := +
ifeq ($(MAKE_RESTARTS),)
clean.inc: fresh
	@printf "clean:%s:%s\n" "$(stage)" "$(stamp)" >> proof; rm -f b.mk
b.mk:: fresh
	@printf "branch-0:%s:%s\n" "$(stage)" "$(stamp)" >> proof; :
b.mk:: fresh
	@printf "branch-1:%s:%s\n" "$(stage)" "$(stamp)" >> proof; :
else
clean.inc: | steady ; @false
endif
all: ; @printf "goal:%s:%s\n" "$(stage)" "$(payload)" > goal-proof
