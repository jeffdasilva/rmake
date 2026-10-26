$(shell python3 setup.py)
P=a\ b
$(info [$(words $(P))|$(word 1,$(P))|$(word 2,$(P))])
all: $(P)
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
