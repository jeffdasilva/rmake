V = global
all: V := selected
peer: V := peer
WRITE = $(eval all: V += rebound)
all: SNAPSHOT := $(eval peer: FIRST := $$(WRITE))$(eval peer: SECOND := $$(WRITE))$(info resumed=[$(V)/$(flavor V)])
all: peer ; @$(info target=[$(V)]):
peer: ; @$(info peer=[$(V)]):
later: ; @$(info global=[$(V)]):
