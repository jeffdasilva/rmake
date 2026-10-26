$(shell python3 setup.py)
all: *.in $(file >late.in,late)
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
