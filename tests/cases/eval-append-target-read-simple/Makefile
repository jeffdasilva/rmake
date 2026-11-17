V = global
all: V := selected
APPEND = $(eval V += $$(file >>seen,RHS=[$$(V)])global-tail)
all: SNAPSHOT := $(APPEND)
all: V += local-tail
all: ; @$(info target=[$(V)/$(flavor V)])cat seen
later: ; @$(info global=[$(V)/$(flavor V)]):
