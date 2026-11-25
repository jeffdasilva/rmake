$(info reader:$(or $(MAKE_RESTARTS),0))
MAKEFLAGS += --debug=b
.PHONY: all left right
all: left right
left: ; @printf 'left\n'
right: ; @printf 'right\n'
