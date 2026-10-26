.DEFAULT_GOAL=all
unexport HOME
HOME=$(if $(wildcard ),late,early)
~/target: X:=$(file >marker,seen)value
all: early/target
early/target:;@printf '[%s|%s]\n' '$@' '$(X)'
