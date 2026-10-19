export X=old
unexport override undefine X
X=later
all:
	@printf "child=<%s>|%s\n" "$$X" "$${X+present}"
