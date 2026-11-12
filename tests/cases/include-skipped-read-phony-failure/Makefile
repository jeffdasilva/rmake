.DEFAULT_GOAL := all
$(file >>seen,read-$(MAKE_RESTARTS))
include skip.mk
include peer.mk
.PHONY: skip.mk
skip.mk: ; @printf "SKIP := incorrect\n" > "$@"
peer.mk: ; @printf "PEER := correct\n" > "$@"
all: ; @printf "ordinary %s %s\n" "$(PEER)" "$(SKIP)"
-include bad.mk
bad.mk: ; @false
