$(shell python3 setup.py)
unexport HOME
HOME=house
.SECONDEXPANSION:
all: $$(addprefix ,old)
all: ~/a
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
