$(shell touch -d @1600000000 base; touch -d @1600000001 leaf; touch -d @1600000002 all)
.PHONY: leaf
all: leaf
	@printf "%s\n" parent >> effects
	@printf "%s\n" parent
leaf:: base
le%:
	@printf "pattern[%s]\n" "$@" >> effects
	@printf "pattern[%s]\n" "$@"
