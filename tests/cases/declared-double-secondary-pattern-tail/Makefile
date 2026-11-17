$(shell touch -d @1600000000 base fixed other gate; touch -d @1600000001 leaf; touch -d @1600000003 fixed input)
.SECONDEXPANSION:
BASE := base
all: leaf
	@printf "parent\n" >> seen
	@cat seen
leaf:: fixed $$(file >>seen,head[$$@|$$<|$$^|$$+|$$*|$$|]) $$(BASE)
	@printf "named[%s][%s][%s]\n" "$@" "$<" "$*" >> seen
leaf:: other $$(file >>seen,tail[$$@|$$<|$$^|$$+|$$*|$$|]) $$(BASE) | gate
le%: input
	@printf "recipe[%s][%s][%s][%s][%s]\n" "$@" "$<" "$^" "$*" "$|" >> seen
