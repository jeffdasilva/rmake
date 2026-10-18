unexport X
undefine X
X ?= later
$(info value=<$(value X)>|$(X)|$(flavor X)|$(origin X))
all:
	@printf 'child-X=<%s>|%s\n' "$$X" "$${X+present}"
