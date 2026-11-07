$(info read $(or $(MAKE_RESTARTS),0))
.PHONY: report
report: ; @printf "ordinary\n"
include inventory.complete
-include inventory.part
%.part %.complete:
	@printf "attempt %s/%s\n" "$@" "$(or $(MAKE_RESTARTS),0)"
	@touch -t 200201020304 "$@"
	@if test "$@" = inventory.part; then touch -t 200301020304 inventory.complete; fi
	@exit 9
