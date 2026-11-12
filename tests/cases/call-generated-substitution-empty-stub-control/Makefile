V := alpha a
NAME := V:a=b
define $(NAME)

endef
VALUE := $(call $(NAME),$(info eager)inner)
$(info value=[$(VALUE)])
all: ; @:
