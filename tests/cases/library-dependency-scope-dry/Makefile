$(shell python3 setup.py)
.LIBPATTERNS=lib%.so lib%.a
-ldemo: X=source
all: -ldemo;@printf 'first=[%s]\n' '$<'
