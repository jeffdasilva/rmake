V = global
all: V := outer
peer: V := inner
APPEND = $(eval V += $$(eval peer: V := rebound)$$(info RHS=[$$(V)])tail)
all: SNAPSHOT := $(eval peer: NESTED := $$(APPEND))$(info return=[$(V)])
all: peer ; @$(info target=[$(V)]):
peer: ; @$(info peer=[$(V)]):
later: ; @$(info global=[$(V)/$(flavor V)]):
