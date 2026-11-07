$(info read=$(or $(MAKE_RESTARTS),0))
ifeq ($(MAKE_RESTARTS),)
$(shell touch -t 200001010000 inventory.complete side.gen)
endif
.PHONY: report force
report: ; @printf "ordinary\n"
include side.gen
side.gen: force
	@printf "side/%s\n" "$(or $(MAKE_RESTARTS),0)"
	@touch -t 200201020304 inventory.complete
-include inventory.part
include inventory.complete
inventory.part inventory.complete &:
	@printf "attempt %s/%s\n" "$@" "$(or $(MAKE_RESTARTS),0)"
	@touch -t 200201020304 inventory.part inventory.complete
	@exit 9
