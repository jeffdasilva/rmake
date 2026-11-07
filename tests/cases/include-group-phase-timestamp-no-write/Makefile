$(info read=$(or $(MAKE_RESTARTS),0))
ifeq ($(MAKE_RESTARTS),)
$(shell touch -t 200001010000 inventory.complete)
endif
.PHONY: report
report: ; @printf "ordinary\n"
-include inventory.part
include inventory.complete
inventory.part inventory.complete &:
	@printf "attempt %s/%s\n" "$@" "$(or $(MAKE_RESTARTS),0)"
	@exit 9
