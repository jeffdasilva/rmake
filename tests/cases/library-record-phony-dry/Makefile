$(shell python3 setup.py)
.LIBPATTERNS=lib%.a
.PHONY: libdemo.a
all: -ldemo;@printf '[%s]\n' '$<'
libdemo.a:;@printf 'dest=[%s]\n' '$@'
