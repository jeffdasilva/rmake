guile=[$(0)|$(1)|$(2)]
$(info feature=[$(filter guile,$(.FEATURES))])
OUT:=$(call   guile suffix,alpha,beta)
$(info restart=[$(MAKE_RESTARTS)] result=[$(OUT)])
include gen.mk
all: ; @true
gen.mk: ; @printf 'READY=yes\n' > $@
