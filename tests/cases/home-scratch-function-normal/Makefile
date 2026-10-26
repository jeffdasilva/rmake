.DEFAULT_GOAL=all
unexport HOME
HOME=$(if $(wildcard marker),late,early)
FOUND:=$(wildcard ~/input)
all:;@printf '[%s]\n' '$(FOUND)'
