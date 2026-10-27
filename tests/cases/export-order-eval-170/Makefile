all: export A = $(info A)$(eval unexport B)a
all: export B = $(info B)b
all: export C = $(info C)c
all:;@python3 probe.py
