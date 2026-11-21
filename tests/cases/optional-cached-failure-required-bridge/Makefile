include a.mk
-include b.mk
a.mk: bridge ; @false
b.mk:: bridge ; @false
b.mk:: bridge ; @false
bridge: missing
all: ; @printf "ordinary\n"
