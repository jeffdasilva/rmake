include a.mk
-include b.mk
a.mk: bridge ; @false
b.mk:: bridge ; @false
b.mk:: bridge ; @false
bridge: deeper
deeper: missing
all: ; @printf "ordinary\n"
