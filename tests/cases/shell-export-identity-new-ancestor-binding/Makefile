export CYCLE = $(eval parent: export CYCLE := replacement)$(shell printf "child=[%s]" "$$CYCLE"; printf "child\n" >>seen)
parent: child;@:
child:;@$(info result=[$(CYCLE)]):
