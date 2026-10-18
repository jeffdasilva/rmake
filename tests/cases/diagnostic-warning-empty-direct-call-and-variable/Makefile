warning = stored
A := $(warning)
B := $(call warning)
C := $(warning )
D := $(call warning,)
all:
	@printf '[%s][%s][%s][%s]\n' '$(A)' '$(B)' '$(C)' '$(D)'
