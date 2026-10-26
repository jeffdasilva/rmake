.DEFAULT_GOAL=all
unexport HOME
HOME=$(if $(wildcard x),late,early)
~: X:=$(file >marker,seen)value
all: early
early:;@printf '[%s|%s]\n' '$@' '$(X)'
