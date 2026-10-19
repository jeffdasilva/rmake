all: first second;@test "$$(cat trace)" = once
first second: shared
shared:
	@sleep .02
	@echo once >> trace
