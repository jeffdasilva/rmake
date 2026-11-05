export CYCLE = $(shell printf "child=[%s]" "$$CYCLE"; printf "child\n" >>seen)
OBS := $(CYCLE)
all:;@$(file >>seen,goal)$(info result=[$(OBS)]):
