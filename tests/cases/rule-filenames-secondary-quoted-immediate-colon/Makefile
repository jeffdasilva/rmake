$(shell python3 setup.py)
.SECONDEXPANSION:
all: two\:words.in $$(strip)
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
