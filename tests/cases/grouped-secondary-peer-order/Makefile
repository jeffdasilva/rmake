.SECONDEXPANSION:
all: a b c
a b c &: $$(info expand $$@)$$(addsuffix .in,$$@);@echo "$@:$^";touch a b c
a.in b.in c.in:;@echo $@;touch $@
