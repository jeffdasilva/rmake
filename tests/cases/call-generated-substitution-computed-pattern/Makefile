V := file% file\%
PATTERN := \%
NAME := V:$(PATTERN)=.out
define $(NAME)
literal-stub
endef
VALUE := $(call $(NAME),inner)
$(info value=[$(VALUE)])
all: ; @:
