export A = $(info E:A)$(file >>journal,A)A
export B = $(info E:B)$(file >>journal,B)B
export C = $(info E:C)$(file >>journal,C)C
export C = $(info E:C)$(error stop-C)
all:;@python3 probe.py
