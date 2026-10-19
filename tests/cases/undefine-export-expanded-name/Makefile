N=X
export X=old
undefine $(N)
X=later
all:
	@printf "child=<%s>|%s\n" "$$X" "$${X+present}"
