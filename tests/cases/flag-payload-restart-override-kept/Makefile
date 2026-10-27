override X=kept
override MAKEFLAGS = X=flag
include rebuilt.mk
$(info restart=[$(MAKE_RESTARTS)] X=[$(X)]/$(origin X))
all: ; @true
rebuilt.mk: ; @printf "READY=yes\n" > $@
