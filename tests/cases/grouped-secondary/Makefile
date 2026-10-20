.SECONDEXPANSION:
all: a b
a b &: $$(info expand $$@)$$(addsuffix .in,$$@);@echo "$@:$^";touch a b
a.in b.in:;@echo $@;touch $@
