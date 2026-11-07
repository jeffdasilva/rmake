include recover.gen
.PHONY: all
all: ; @printf "ordinary\n"
ifeq ($(MAKE_RESTARTS),)
recover.gen:
	-@./missing-program
	@printf "loaded := yes\n" > "$@"
endif
