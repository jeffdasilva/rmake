$(info feature=[$(filter guile,$(.FEATURES))])
OUT:=$(if ,$(guile $(error unused)),SAFE)
$(info restart=[$(MAKE_RESTARTS)] result=[$(OUT)])
include gen.mk
all: ; @true
gen.mk: ; @printf 'READY=yes\n' > $@
