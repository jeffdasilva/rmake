V := alpha a
NAME := 2:a=b
define $(NAME)
literal-stub
endef
CALLER = $(call $(NAME))|$(2)
VALUE := $(call CALLER,left,right)
$(info value=[$(VALUE)])
all: ; @:
