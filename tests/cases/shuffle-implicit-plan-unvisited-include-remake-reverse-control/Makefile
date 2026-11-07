all: ; @printf 'goal restart=%s generated=%s\n' '$(MAKE_RESTARTS)' '$(GENERATED)'
include generated.mk
generated.mk: d01 d02 d03 d04 d05 d06 d07 d08 d09 d10 d11
%.mk: ; @printf 'GENERATED := loaded\n' > '$@'; printf 'include %s\n' '$@'
d01 d02 d03 d04 d05 d06 d07 d08 d09 d10 d11: ; @printf 'remake-leaf %s\n' '$@'; touch '$@'
