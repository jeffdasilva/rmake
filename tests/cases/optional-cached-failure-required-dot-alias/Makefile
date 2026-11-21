include a.mk
-include b.mk
a.mk: ./missing ; @false
b.mk:: missing ; @false
b.mk:: missing ; @false
all: ; @printf "ordinary\n"
