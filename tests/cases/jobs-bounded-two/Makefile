all: a b c d
	@test "$$(cat maximum)" = 2 && test "$$(cat active)" = 0
a b c d:;@python3 worker.py $@
