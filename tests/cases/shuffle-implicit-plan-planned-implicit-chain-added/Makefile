all.out: d01 d02 d03 d04 d05 d06 d07 d08 d09 d10 d11
%.out: %.mid ; @printf 'implicit %s [%s] [%s]\n' '$@' '$^' '$|'
%.mid: %.in ; @printf 'intermediate %s\n' '$@'; touch '$@'
all.in: ; @printf 'source %s\n' '$@'; touch '$@'
d01 d02 d03 d04 d05 d06 d07 d08 d09 d10 d11: ; @printf 'leaf %s\n' '$@'
