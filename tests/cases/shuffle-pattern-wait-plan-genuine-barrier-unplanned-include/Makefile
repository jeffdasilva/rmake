all: ; @printf 'restart=%s\n' '$(MAKE_RESTARTS)'
include generated.mk
generated.mk: d01 d02 d03 d04 d05 d06 d07 d08 d09 d10 d11
%.mk: p1 .WAIT p2 ; @printf 'VAR := loaded\n' >'$@'
p1 p2 d01 d02 d03 d04 d05 d06 d07 d08 d09 d10 d11: ; @printf 'remake %s\n' '$@'; touch '$@'
