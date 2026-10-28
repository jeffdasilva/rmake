NAME:=guile form
$(NAME)=FILE
$(info feature=[$(filter guile,$(.FEATURES))])
OUT:=$(guile $(file >observed,name-effect)form)
$(info restart=[$(MAKE_RESTARTS)] result=[$(OUT)])
include gen.mk
all: ; @true
gen.mk: ; @printf 'READY=yes\n' > $@
