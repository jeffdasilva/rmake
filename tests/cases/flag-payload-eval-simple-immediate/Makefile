X := original
$(info before X=[$(X)]/[$(value X)]/$(origin X)/$(flavor X) flags=[$(MAKEFLAGS)] over=[$(MAKEOVERRIDES)])
all:
	@$(eval MAKEFLAGS = X:::=value)$(info after X=[$(X)]/[$(value X)]/$(origin X)/$(flavor X) flags=[$(MAKEFLAGS)] over=[$(MAKEOVERRIDES)])true
