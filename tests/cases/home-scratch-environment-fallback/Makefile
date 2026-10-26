.DEFAULT_GOAL=all
ORIGINAL_HOME:=$(shell printf '%s' "$$HOME")
unexport HOME
HOME=$(wildcard marker)
~/target: X:=value
all:$(ORIGINAL_HOME)arker
$(ORIGINAL_HOME)arker:;@printf '[%s|%s]\n' '$(notdir $@)' '$(X)'
