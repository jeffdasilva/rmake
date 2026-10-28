$(info feature=[$(filter guile,$(.FEATURES))])
OUT:=$(guile (+ 1 2))
$(info restart=[$(MAKE_RESTARTS)] result=[$(OUT)])
include gen.mk
all: ; @true
gen.mk: ; @printf 'READY=yes\n' > $@
