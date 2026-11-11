V := alpha a
NAME := V=a:b
define $(NAME)
$(1)-literal
endef
VALUE := $(call $(NAME),inner)
$(info value=[$(VALUE)])
all: ; @:
