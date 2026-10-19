override X=later
$(info parse=<$(X)>|$(origin X)|$(flavor X))
all:
	@printf "make=<%s>|%s|%s\nchild=<%s>|%s\n" "$(X)" "$(origin X)" "$(flavor X)" "$$X" "$${X+present}"
