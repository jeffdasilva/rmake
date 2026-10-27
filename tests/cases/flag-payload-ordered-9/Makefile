MAKEFLAGS = X:=one -Iinc Y:=$$(filter\ inc,$$(.INCLUDE_DIRS))
$(info after X=[$(X)]/[$(value X)] Y=[$(Y)] Z=[$(Z)] flags=[$(MAKEFLAGS)] over=[$(MAKEOVERRIDES)])
all: ; @true
