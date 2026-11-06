export CYCLE = $(eval owner: CAP := $$(shell printf "inner=[%s]" "$$$$CYCLE"; printf "inside\n" >>seen))outer
all:;@$(info result=[$(CYCLE)]):
owner:;@$(info cap=[$(CAP)]):
