.DEFAULT_GOAL=all
unexport HOME
HOME=$(if $(wildcard x),late,early)
~/lib(a b): X:=value
all: early .WAIT x
early x:;@printf '[%s|%s]\n' '$@' '$(X)'
