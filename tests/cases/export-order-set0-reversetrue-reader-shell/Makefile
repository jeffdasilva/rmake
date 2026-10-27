export C = $(info E:C)$(file >>journal,C)C
export B = $(info E:B)$(file >>journal,B)B
export A = $(info E:A)$(file >>journal,A)A
RESULT := $(shell printf reader)
all:
	@python3 probe.py
