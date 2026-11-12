$(file >seen,initial)
EMPTY :=
NAMES := left right
left = outside-left
right := outside-right
invoke = $(let 1 2,,[$(1)][$(2)])[$(1)][$(2)]
.PHONY: check
check: ; @$(info result=[$(let left right,$(EMPTY),[$(left)][$(right)])] outside=[$(left)][$(right)]):
