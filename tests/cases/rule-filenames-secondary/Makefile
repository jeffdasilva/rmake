$(shell python3 setup.py)
.SECONDEXPANSION:
P=two\ words.in
all: $$(P)
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
