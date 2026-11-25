$(info reader:$(or $(MAKE_RESTARTS),0))
include stage.inc
stage.inc:
	@printf '%s\n' '# completed' > stage.inc
all: ; @printf "goal:%s\n" "$(or $(MAKE_RESTARTS),0)"
