$(file >seen,initial)
EMPTY :=
NAMES := left right
left = outside-left
right := outside-right
invoke = $(let 1 2,,[$(1)][$(2)])[$(1)][$(2)]
.PHONY: check
check: ; @$(info result=[$(call invoke,first,second)] outside=[$(left)][$(right)]):
