N=$(warning macro)Y
MAKEOVERRIDES=$$(N)=body
all:
	@$(info make-Y=<$(value Y)>|$(origin Y)|$(flavor Y))printf "child=<%s>|%s\n" "$$Y" "$${Y+present}"
