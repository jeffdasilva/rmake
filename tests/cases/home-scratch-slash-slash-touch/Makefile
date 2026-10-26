.DEFAULT_GOAL=all
unexport HOME
HOME=$(if $(wildcard one/two),late,early)
~/: X:=$(file >marker,seen)value
all: earlyne/two
earlyne/two:;@printf '[%s|%s]\n' '$@' '$(X)'
