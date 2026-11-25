$(info reader:$(or $(MAKE_RESTARTS),0))
GNUMAKEFLAGS += --shuffle=+00007
.PHONY: all left right
all: left right
left: ; @printf 'left\n'
right: ; @printf 'right\n'
