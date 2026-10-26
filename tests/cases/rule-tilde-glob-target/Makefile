$(shell python3 setup.py)
unexport HOME
HOME=house
.PHONY: all house/a.out house/b.out
all: house/a.out house/b.out
~/*.out:
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
