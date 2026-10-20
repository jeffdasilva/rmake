all: a b
a b a &:: input | order;@echo "$@:$^:$+:$|";touch -d @1000000000 a b
input order:;@touch $@
