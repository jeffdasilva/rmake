NAME:=guile form
$(NAME)=SHELL
$(info feature=[$(filter guile,$(.FEATURES))])
OUT:=$(guile $(shell printf %s form))
$(info restart=[$(MAKE_RESTARTS)] result=[$(OUT)])
include gen.mk
all: ; @true
gen.mk: ; @printf 'READY=yes\n' > $@
