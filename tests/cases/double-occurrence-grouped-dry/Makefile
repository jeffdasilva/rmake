all: a b
a b a &:: input;@echo "$@:$+";touch -d @1000000000 a b
input:;@touch input
