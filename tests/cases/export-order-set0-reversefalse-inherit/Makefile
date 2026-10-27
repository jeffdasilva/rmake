parent: export A = $(info E:A)$(file >>journal,A)A
parent: export B = $(info E:B)$(file >>journal,B)B
parent: export C = $(info E:C)$(file >>journal,C)C
parent: all
all:
	@python3 probe.py
