$(shell mkdir d)
.LIBPATTERNS=lib%.a
VPATH=d
all: -ldemo;@printf 'all=[%s]\n' '$<'
d/libdemo.a:;@printf 'dest=[%s]\n' '$@'
