$(shell python3 setup.py)
all: foo.out
foo.out: %.out: literal\%.in
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
