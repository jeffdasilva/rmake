.SECONDEXPANSION:
all: a b
a b a &:: $$(info expand $$@:$$+)$$(addsuffix .in,$$@);@echo "$@:$+";touch -d @1000000000 a b
a.in b.in:;@echo $@;touch $@
