all:
	-@./runner
	@test "$$(stat -c %a ./runner)" = 655 && printf "continued\n"
