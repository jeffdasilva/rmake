.DEFAULT_GOAL=all
unexport HOME
HOME=$(if $(wildcard first second),late,early)
~/target: X:=$(file >marker,seen)value
all: earlyecond
earlyecond:;@printf '[%s|%s]\n' '$@' '$(X)'
