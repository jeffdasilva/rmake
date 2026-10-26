$(shell python3 setup.py)
unexport HOME
HOME=$(warning home)house
.SECONDEXPANSION:
P=~/a
all: $$(P)
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
