all: target;@test "$$(cat target)" = firstsecond
target:
	@printf first > target;sleep .02
	@printf second >> target
