all: left right;@test "$$(cat left)" = leftleft && test "$$(cat right)" = rightright
.DEFAULT:
	@printf "%s" "$@" > $@
	@printf "%s" "$<" >> $@
