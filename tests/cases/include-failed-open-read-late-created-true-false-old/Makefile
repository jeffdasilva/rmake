all: ; @printf "payload=[%s]\n" "$(PAYLOAD)"
include payload.mk
payload.mk:
	@rm -f "$@"
	@printf "PAYLOAD := repaired\n" > "$@"
$(file >payload.mk,PAYLOAD := too late)
