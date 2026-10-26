$(shell python3 setup.py)
unexport HOME
HOME=house
.SECONDEXPANSION:
P=~/*.in
all: $$(P)
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
