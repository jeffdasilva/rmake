V = global
all: V := before
APPEND = $(eval V += $$(eval all: V := rebound)$$(info RHS=[$$(V)])tail)
all: SNAPSHOT := $(eval all: NESTED := $$(APPEND))$(info return=[$(V)])
all: ; @$(info target=[$(V)]):
later: ; @$(info global=[$(V)/$(flavor V)]):
