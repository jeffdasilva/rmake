$(shell python3 setup.py)
.SECONDEXPANSION:
P=\ leading.in
all: x.out
%.out: $$(P)
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
