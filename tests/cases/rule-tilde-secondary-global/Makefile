$(shell python3 setup.py)
unexport HOME
HOME=global
.SECONDEXPANSION:
P=~/a
all: HOME=local
all: $$(P)
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
