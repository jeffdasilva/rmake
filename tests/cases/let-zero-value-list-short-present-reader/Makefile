$(file >seen,initial)
EMPTY :=
NAMES := left right
left = outside-left
right := outside-right
invoke = $(let 1 2,,[$(1)][$(2)])[$(1)][$(2)]
RESULT := $(let left right,pear,[$(left)][$(right)])
$(info result=[$(RESULT)] outside=[$(left)][$(right)])
.PHONY: check
check: ; @:
