ifeq ($(MAKE_RESTARTS),)
$(shell chmod 000 search/payload.mk; if test -r search/payload.mk; then chmod 644 search/payload.mk; printf "PAYLOAD := repaired\n" > search/payload.mk; fi)
endif
all: ; @printf "payload=[%s]\n" "$(PAYLOAD)"
-include payload.mk
search/payload.mk:
	@rm -f "$@"
	@printf "PAYLOAD := repaired\n" > "$@"
