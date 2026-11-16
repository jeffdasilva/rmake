$(shell touch -d @1600000000 base fixed other gate; touch -d @1600000001 leaf)
.SECONDEXPANSION:
BASE := base
all: leaf
	@printf "parent\n" >> seen
	@cat seen
	@cat input
leaf:: $$(file >>seen,created[$$@|$$<|$$*]) $$(file >input,late) $$(BASE)
le%: input
	@printf "recipe[%s][%s][%s][%s][%s]\n" "$@" "$<" "$^" "$*" "$|" >> seen
