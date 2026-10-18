all: left right
	@test -f left && test -f right
	@printf '%s\n' complete
left right:
	@printf '%s\n' '$@' > $@
