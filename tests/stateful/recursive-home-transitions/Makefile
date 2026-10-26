.DEFAULT_GOAL=all
unexport HOME
include config.mk
~/target: X:=value
all:$(GOAL)
$(GOAL):;@printf '[%s|%s]\n' '$@' '$(X)'
