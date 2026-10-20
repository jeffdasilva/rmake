.DELETE_ON_ERROR:
all: a b
a b a &:: input;@touch -d @1000000000 a b;exit 7
input:;@touch input
