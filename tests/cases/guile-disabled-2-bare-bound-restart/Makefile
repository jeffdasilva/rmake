guile=BARE
$(info feature=[$(filter guile,$(.FEATURES))])
OUT:=$(guile)
$(info restart=[$(MAKE_RESTARTS)] result=[$(OUT)])
include gen.mk
all: ; @true
gen.mk: ; @printf 'READY=yes\n' > $@
