all: a b
a b &: base;@echo "$@:$<:$^:$?";touch a b
b: extra
base extra:;@echo $@;touch $@
