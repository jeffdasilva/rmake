NAMES=X Y X
export $(NAMES)
X ?= ignored
Y += tail
$(info X=<$(value X)>|$(X)|$(flavor X)|$(origin X))
$(info Y=<$(value Y)>|$(Y)|$(flavor Y)|$(origin Y))
all:
	@printf 'child-X=<%s>|%s\n' "$$X" "$${X+present}"
	@printf 'child-Y=<%s>|%s\n' "$$Y" "$${Y+present}"
