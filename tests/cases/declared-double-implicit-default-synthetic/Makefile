$(shell touch -d @1600000000 base other; touch -d @1600000001 leaf; touch -d @1600000000 input)
all: leaf
	@printf "parent\n" >> effects
	@cat effects
leaf:: base
.DEFAULT:
	@printf "default[%s][%s]\n" "$@" "$<" >> effects
