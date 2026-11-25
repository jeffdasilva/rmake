$(info reader:$(or $(MAKE_RESTARTS),0))
.PHONY: all left right
all: left right
left: ; @printf 'left\n'
right: ; $(eval MAKEFLAGS += --shuffle=identity) @printf 'right\n'
