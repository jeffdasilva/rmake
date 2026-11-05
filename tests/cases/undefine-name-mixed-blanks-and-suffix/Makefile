PREFIX := $(file <prefix)
TAIL := $(file <tail)
SUFFIX := $(file <suffix)
NAME := X$(SUFFIX)
X := plain
$(NAME) := distinct
undefine $(PREFIX)$(NAME)$(TAIL)
$(info after=[$(X)/$(origin $(NAME))])
all:;@:
