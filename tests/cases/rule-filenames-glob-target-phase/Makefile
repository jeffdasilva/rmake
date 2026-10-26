$(shell python3 setup.py)
.PHONY: all old.out late.out
all: old.out late.out
*.out: $(file >late.out,late)
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
late.out:;@printf "late-explicit\n"
