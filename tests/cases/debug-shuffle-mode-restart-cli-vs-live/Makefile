$(info reader:$(or $(MAKE_RESTARTS),0))
ifeq ($(MAKE_RESTARTS),)
MAKEFLAGS += --shuffle=ReVeRsE
endif
include stage.inc
stage.inc:
	@printf '%s\n' '# completed' > stage.inc
.PHONY: all left right
all: left right
left: ; @printf 'left\n'
right: ; @printf 'right\n'
