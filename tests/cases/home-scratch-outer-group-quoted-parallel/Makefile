.DEFAULT_GOAL=all
unexport HOME
HOME=$(if $(wildcard a\ b),late,early)
~/lib(a b): X:=value
all: early\ b .WAIT a\ b
early\ b a\ b:;@printf '[%s|%s]\n' '$@' '$(X)'
