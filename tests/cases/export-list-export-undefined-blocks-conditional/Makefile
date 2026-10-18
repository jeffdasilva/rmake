export X
$(info before=<$(value X)>|$(X)|$(flavor X)|$(origin X))
X ?= ignored
$(info after=<$(value X)>|$(X)|$(flavor X)|$(origin X))
all:
	@printf 'child-X=<%s>|%s\n' "$$X" "$${X+present}"
