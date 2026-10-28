NAME:=guile form
$(info feature=[$(filter guile,$(.FEATURES))])
OUT:=$(guile $(eval $$(NAME)=EVAL)form)
$(info restart=[$(MAKE_RESTARTS)] result=[$(OUT)])
include gen.mk
all: ; @true
gen.mk: ; @printf 'READY=yes\n' > $@
