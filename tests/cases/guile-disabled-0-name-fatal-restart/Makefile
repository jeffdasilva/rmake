$(info feature=[$(filter guile,$(.FEATURES))])
OUT:=$(guile $(error name-fail))
$(info restart=[$(MAKE_RESTARTS)] result=[$(OUT)])
include gen.mk
all: ; @true
gen.mk: ; @printf 'READY=yes\n' > $@
