$(info read $(or $(MAKE_RESTARTS),0))
.PHONY: report
report: ; @printf "ordinary\n"
-include inventory.part
include inventory.complete
%.part %.complete:
	@printf "attempt %s/%s\n" "$@" "$(or $(MAKE_RESTARTS),0)"
	@touch -t 200201020304 "$@"
	@exit 9
