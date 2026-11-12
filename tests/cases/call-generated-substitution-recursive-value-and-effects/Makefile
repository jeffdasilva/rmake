V = $(info selected)$(1)a
NAME := V:a=b
define $(NAME)
$(error original-stub)
endef
VALUE := $(call $(NAME),$(info eager)inner)
$(info value=[$(VALUE)])
all: ; @:
