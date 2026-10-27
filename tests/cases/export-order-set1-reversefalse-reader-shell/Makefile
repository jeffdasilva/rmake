export Z = $(info E:Z)$(file >>journal,Z)Z
export X = $(info E:X)$(file >>journal,X)X
export Y = $(info E:Y)$(file >>journal,Y)Y
RESULT := $(shell printf reader)
all:
	@python3 probe.py
