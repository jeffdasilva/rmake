$(shell test -f seed || { :; : >seed; })
-include fragment.mk
.DEFAULT_GOAL := all
all:
	@test "$(LEFT)" = left && test "$(RIGHT)" = "right" && printf 'ordinary=%s:%s\n' "$(LEFT)" "$(RIGHT)"
fragment.mk:: left ; @printf 'LEFT := left\n' >fragment.mk; touch -d @1600000003 fragment.mk
fragment.mk:: right ; @printf 'RIGHT := right\n' >>fragment.mk; touch -d @1600000003 fragment.mk
left right: ;
