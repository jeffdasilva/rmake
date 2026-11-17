V = global
all: V := selected
peer: V := peer
WRITE = $(eval all: V := rebound)$(info inner=[$(V)])
all: SNAPSHOT := $(eval peer: INNER := $$(WRITE))$(file >>seen,resumed=[$(V)])
all: peer ; @$(info target=[$(V)])cat seen
peer: ; @$(info peer=[$(V)]):
later: ; @$(info global=[$(V)]):
