parent: export Z = $(info E:Z)$(file >>journal,Z)Z
parent: export X = $(info E:X)$(file >>journal,X)X
parent: export Y = $(info E:Y)$(file >>journal,Y)Y
parent: private Z = $(info private)Z
parent: all
all:
	@python3 probe.py
