$(shell python3 setup.py)
.SECONDEXPANSION:
all: two\ words.out
two\ words.out: %.out: $$(addsuffix .in,$$*)
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
