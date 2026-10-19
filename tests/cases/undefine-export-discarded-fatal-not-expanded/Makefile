export X=old
undefine X
X=$(file >must-not-exist)$(error must-not-expand)
all:
	@printf "child=<%s>|%s\n" "$$X" "$${X+present}"
