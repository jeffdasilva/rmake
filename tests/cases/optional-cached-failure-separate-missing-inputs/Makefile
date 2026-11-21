include a.mk
-include b.mk
a.mk: missing-a ; @false
b.mk:: missing-b ; @false
b.mk:: missing-b ; @false
all: ; @printf "ordinary\n"
