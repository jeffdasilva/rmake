all: before bad after
before:
	@printf kept > before
	@printf 'before\n'
bad:
	@$(error stop)
after:
	@touch forbidden
