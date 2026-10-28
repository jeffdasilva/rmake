NAME:=guile form
$(NAME)=$(error value-fail)
$(info feature=[$(filter guile,$(.FEATURES))])
OUT:=$(guile form)
$(info restart=[$(MAKE_RESTARTS)] result=[$(OUT)])
include gen.mk
all: ; @true
gen.mk: ; @printf 'READY=yes\n' > $@
