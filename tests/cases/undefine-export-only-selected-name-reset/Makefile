export X=old
export Y=retained
undefine X
X=later
all:
	@printf "X=<%s>|%s Y=<%s>|%s\n" "$$X" "$${X+present}" "$$Y" "$${Y+present}"
