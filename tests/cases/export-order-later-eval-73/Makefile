all: export A = $(info A)$(eval C = $(info changed-C)$(warning changed-C)new)a
all: export B = $(info B)b
all: export C = $(info C)c
all:;@python3 probe.py
