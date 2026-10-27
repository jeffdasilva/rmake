MAKEFLAGS += X:=one\ two
$(info after X=[$(X)]/[$(value X)]/$(origin X)/$(flavor X) flags=[$(MAKEFLAGS)] over=[$(MAKEOVERRIDES)])
all: ; @$(info recipe X=[$(X)]/[$(value X)]/$(origin X)/$(flavor X) flags=[$(MAKEFLAGS)] over=[$(MAKEOVERRIDES)])true
