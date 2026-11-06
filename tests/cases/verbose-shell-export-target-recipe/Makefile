all: export CYCLE = $(shell printf "child=[%s]" "$$CYCLE"; printf "child\n" >>seen)
all:;@$(info result=[$(CYCLE)]):
