.DEFAULT_GOAL=all
unexport HOME
HOME=$(if $(wildcard a\ b),late,early)
~/: X:=$(file >marker,seen)value
all: early\ b
early\ b:;@printf '[%s|%s]\n' '$@' '$(X)'
