.SECONDEXPANSION:
all: $$(LATE)
LATE = inner.out
inner.out: d01 d02 d03 d04 d05 d06 d07 d08 d09 d10 d11
%.out: ; @printf 'implicit %s [%s] [%s]\n' '$@' '$^' '$|'
d01 d02 d03 d04 d05 d06 d07 d08 d09 d10 d11: ; @printf 'leaf %s\n' '$@'
