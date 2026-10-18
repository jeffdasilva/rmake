export $(NAMES)
$(info X=<$(value X)>|$(X)|$(flavor X)|$(origin X))
$(info Y=<$(value Y)>|$(Y)|$(flavor Y)|$(origin Y))
$(info Z=<$(value Z)>|$(Z)|$(flavor Z)|$(origin Z))
$(info Q=<$(value Q)>|$(Q)|$(flavor Q)|$(origin Q))
$(info R=<$(value R)>|$(R)|$(flavor R)|$(origin R))
$(info S=<$(value S)>|$(S)|$(flavor S)|$(origin S))
all:
	@printf 'child-X=<%s>|%s\n' "$$X" "$${X+present}"
	@printf 'child-Y=<%s>|%s\n' "$$Y" "$${Y+present}"
	@printf 'child-Z=<%s>|%s\n' "$$Z" "$${Z+present}"
	@printf 'child-Q=<%s>|%s\n' "$$Q" "$${Q+present}"
	@printf 'child-R=<%s>|%s\n' "$$R" "$${R+present}"
	@printf 'child-S=<%s>|%s\n' "$$S" "$${S+present}"
