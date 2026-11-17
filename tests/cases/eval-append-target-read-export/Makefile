all: V := selected
all: SNAPSHOT := $(eval export V += tail)
all: ; @printf 'target child=[%s]\n' "$$V"
later: ; @$(info global=[$(V)/$(flavor V)])printf 'global child=[%s]\n' "$$V"
