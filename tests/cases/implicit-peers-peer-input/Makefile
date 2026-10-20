all: x.a x.b
x.b: extra
%.a %.b: base;@echo "$@:$^:$?";touch x.a x.b
base extra:;@echo $@;touch $@
