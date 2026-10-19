Y=early
unexport define X
body$(Y)
endef
Y=late
$(info raw=<$(value X)>|$(origin X)|$(flavor X))
all:;@$(info expanded=<$(X)>)printf "child=<%s>|%s\n" "$$X" "$${X+present}"
