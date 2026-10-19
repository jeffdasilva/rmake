export
unexport X
undefine X
export X=first
undefine X
X=second
all:
	@printf "child=<%s>|%s\n" "$$X" "$${X+present}"
