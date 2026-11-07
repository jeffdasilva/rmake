.PHONY: report
report: task.part
-include task.part
%.part %.index:
	@touch "$@"
	@printf "phase %s\n" \
	  one
	@exit 7
