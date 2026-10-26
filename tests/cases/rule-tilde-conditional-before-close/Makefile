$(shell python3 setup.py)
unexport HOME
HOME=house
all: ~/a
ifeq ($(eval HOME=other),)
endif
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
