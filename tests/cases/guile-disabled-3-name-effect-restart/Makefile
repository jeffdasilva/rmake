NAME:=guile form
$(NAME)=FOUND
$(info feature=[$(filter guile,$(.FEATURES))])
OUT:=$(guile $(info name-effect)form)
$(info restart=[$(MAKE_RESTARTS)] result=[$(OUT)])
include gen.mk
all: ; @true
gen.mk: ; @printf 'READY=yes\n' > $@
