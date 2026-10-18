K=before
unexport X
X += $(K)
K=after
$(info value=<$(value X)>|$(X)|$(flavor X)|$(origin X))
all:
	@printf 'child-X=<%s>|%s\n' "$$X" "$${X+present}"
