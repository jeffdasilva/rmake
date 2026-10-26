$(shell python3 setup.py)
.LIBPATTERNS=lib%.a
all: -ldemo libdemo.a;@printf '[%s|%s]\n' '$<' '$^'
libdemo.a: X=dest
libdemo.a:;@printf 'dest=%s\n' '$(X)'
