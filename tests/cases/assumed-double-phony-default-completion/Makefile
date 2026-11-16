$(shell touch -d @1600000000 base; touch -d @1600000001 leaf; touch -d @1600000002 all)
.PHONY: leaf
all: leaf
	@printf "%s\n" parent >> effects
	@printf "%s\n" parent
leaf:: base
.DEFAULT:
	@printf "default[%s]\n" "$@" >> effects
	@printf "default[%s]\n" "$@"
