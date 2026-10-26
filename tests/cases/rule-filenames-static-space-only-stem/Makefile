$(shell python3 setup.py)
all: \ .out
\ .out: %.out: %.in
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
