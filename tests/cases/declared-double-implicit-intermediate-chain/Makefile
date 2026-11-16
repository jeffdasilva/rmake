$(shell touch -d @1600000000 base other; touch -d @1600000001 leaf; touch -d @1600000003 material)
all: leaf
	@printf "parent\n" >> effects
	@cat effects
leaf:: base
le%: input-%
	@printf "pattern[%s][%s][%s][%s]\n" "$@" "$<" "$^" "$*" >> effects
input-%: material
	@printf "chain[%s][%s][%s]\n" "$@" "$<" "$*" >> effects
	@cp "$<" "$@"
