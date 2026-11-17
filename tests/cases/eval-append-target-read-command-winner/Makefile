all: override V := selected
APPEND = $(eval V += $$(file >>seen,RHS=[$$(V)])ignored)
all: SNAPSHOT := $(APPEND)
all: ; @$(info target=[$(V)])cat seen
later: ; @$(info global=[$(V)/$(origin V)/$(flavor V)]):
