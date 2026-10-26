$(shell python3 setup.py)
unexport HOME
HOME=$(info home)house
all: a.out b.out
a.out b.out: %.out: ~/%.in
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
