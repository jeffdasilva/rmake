export CYCLE = $(eval all: export CYCLE := replacement)$(shell printf "child=[%s]" "$$CYCLE"; printf "child\n" >>seen)
all:;@$(info result=[$(CYCLE)]):
