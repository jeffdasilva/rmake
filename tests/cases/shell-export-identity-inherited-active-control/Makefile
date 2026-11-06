parent: export CYCLE = $(shell printf "child=[%s]" "$$CYCLE"; printf "child\n" >>seen)
parent: child;@:
child:;@$(foreach LOCAL,loop,$(info result=[$(CYCLE)] lexical=[$(LOCAL)])):
