$(shell touch -d @1600000000 base fixed other gate; touch -d @1600000001 leaf; touch -d @1600000003 input)
.SECONDEXPANSION:
BASE := base
all: leaf
	@printf "parent\n" >> seen
	@cat seen
leaf:: fixed $$(file >>seen,declared[$$@|$$<|$$^|$$+|$$*|$$|]) $$(BASE) | gate
leaf:: other $$(file >>seen,second[$$@|$$<|$$^|$$+|$$*|$$|]) $$(BASE)
le%: input $$(file >>seen,implicit[$$@|$$<|$$^|$$+|$$*|$$|])
	@printf "recipe[%s][%s][%s][%s][%s]\n" "$@" "$<" "$^" "$*" "$|" >> seen
