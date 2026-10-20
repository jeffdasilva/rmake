.SECONDEXPANSION:
all: a b
a b a &:: $$(info expand $$@)input;@echo "$@:$+";touch -d @1000000000 a b
input:;@echo input;touch input
