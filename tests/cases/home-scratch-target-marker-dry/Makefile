.DEFAULT_GOAL=all
unexport HOME
HOME=$(if $(wildcard marker),late,early)
~/target: X:=$(file >marker,seen)value
all: earlyarker
earlyarker:;@printf '[%s|%s]\n' '$@' '$(X)'
