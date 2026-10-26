unexport HOME
HOME=$(info HOME)house
define R
out: ~/a
ifeq ($(info CONDITIONAL),)
endif
endef
all:;@printf '%s' '$(eval $(value R))'
