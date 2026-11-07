ifeq ($(MAKE_RESTARTS),)
$(shell chmod 000 payload.mk; if test -r payload.mk; then chmod 644 payload.mk; printf "PAYLOAD := repaired\n" > payload.mk; fi)
endif
all: ; @printf "payload=[%s]\n" "$(PAYLOAD)"
-include payload.mk
payload.mk:
	@rm -f "$@"
	@printf "PAYLOAD := repaired\n" > "$@"
