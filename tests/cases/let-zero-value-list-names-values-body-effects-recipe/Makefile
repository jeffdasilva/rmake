$(file >seen,initial)
EMPTY :=
NAMES := left right
left = outside-left
right := outside-right
invoke = $(let 1 2,,[$(1)][$(2)])[$(1)][$(2)]
.PHONY: check
check: ; @$(info result=[$(let $(file >>seen,names)left right,$(file >>seen,values),$(file >>seen,body)[$(left)][$(right)])] outside=[$(left)][$(right)]):
