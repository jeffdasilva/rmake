$(shell python3 setup.py)
unexport HOME
HOME=house
all: house/a.out
house/a.out: house/%.out: ~/%.in
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
