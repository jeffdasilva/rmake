$(shell python3 setup.py)
all: .out
.out: %.out: pre%.in
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
