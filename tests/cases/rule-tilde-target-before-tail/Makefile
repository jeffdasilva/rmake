$(shell python3 setup.py)
unexport HOME
HOME=early
~/target: $(eval HOME=late) ~/a
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
