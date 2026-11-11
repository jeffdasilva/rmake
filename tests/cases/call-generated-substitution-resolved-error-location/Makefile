V = $(error chosen-error)
NAME := V:a=b
define $(NAME)
$(error original-stub)
endef
VALUE := $(call $(NAME),inner)
$(info value=[$(VALUE)])
all: ; @:
