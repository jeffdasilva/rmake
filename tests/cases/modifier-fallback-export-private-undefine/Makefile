export X=old
export private undefine X
X=later
all:
	@printf "child=<%s>|%s\n" "$$X" "$${X+present}"
