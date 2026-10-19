M=$(warning macro)body
MAKEOVERRIDES=Y:=$$(M)
all:
	@$(info make-Y=<$(value Y)>|$(origin Y)|$(flavor Y))printf "child=<%s>|%s\n" "$$Y" "$${Y+present}"
