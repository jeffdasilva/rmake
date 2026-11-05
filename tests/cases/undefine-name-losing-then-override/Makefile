NAME := $(file <bytes)
override X := held
undefine $(NAME)
$(info kept=[$(X)/$(origin X)])
override undefine $(NAME)
$(info removed=[$(origin X)])
all:;@:
