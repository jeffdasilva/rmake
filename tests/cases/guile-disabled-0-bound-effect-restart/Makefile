NAME:=guile form
$(NAME)=$(info value-effect)VALUE
$(info feature=[$(filter guile,$(.FEATURES))])
OUT:=$(guile form)
$(info restart=[$(MAKE_RESTARTS)] result=[$(OUT)])
include gen.mk
all: ; @true
gen.mk: ; @printf 'READY=yes\n' > $@
