.PHONY: report
report: catalog.fragment
	@printf "ordinary\n"
%.fragment %.metadata:
	@touch "$@"
	@printf "dispatch\n"
	@exit 9
