all.out: e1 e2 e3 e4 e5 e6 e7 e8 e9 e10 e11
include generated.mk
generated.mk: d01 d02 d03 d04 d05 d06 d07 d08 d09 d10 d11
%.mk: ; @printf 'GENERATED := loaded\n' > '$@'; printf 'include %s\n' '$@'
%.out: ; @printf 'goal %s restart=%s generated=%s\n' '$@' '$(MAKE_RESTARTS)' '$(GENERATED)'
d01 d02 d03 d04 d05 d06 d07 d08 d09 d10 d11: ; @printf 'remake-leaf %s\n' '$@'; touch '$@'
e1 e2 e3 e4 e5 e6 e7 e8 e9 e10 e11: ; @printf 'goal-leaf %s\n' '$@'
