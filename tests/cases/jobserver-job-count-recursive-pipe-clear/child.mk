.PHONY: all a b
all: a b
a:
	@set -eu; : > a.started; attempts=0; while test ! -f b.started; do attempts=$$((attempts+1)); test $$attempts -lt 100; sleep 0.03; done; printf 'a\n' > a.finished
b:
	@set -eu; : > b.started; attempts=0; while test ! -f a.started; do attempts=$$((attempts+1)); test $$attempts -lt 100; sleep 0.03; done; printf 'b\n' > b.finished
