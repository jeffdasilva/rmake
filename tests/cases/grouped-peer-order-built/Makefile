all: a b
a b &: own;@echo "$^:$+:$|";touch a b
b: | extra
own extra:;@echo $@;touch $@
