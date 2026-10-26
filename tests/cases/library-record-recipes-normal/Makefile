$(shell python3 setup.py)
.LIBPATTERNS=lib%.a
.PHONY: force
all: -ldemo libdemo.a;@printf '[%s|%s]\n' '$<' '$^'
-ldemo: X=source
libdemo.a: X=dest
-ldemo: force;@printf 'source=[%s|%s]\n' '$@' '$(X)'
libdemo.a:;@printf dest
