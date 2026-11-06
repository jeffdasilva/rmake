NAME := GLOBAL_SLOT
all: NAME := LOCAL_SLOT
all: child
child:;@$(eval child: $$(NAME) := picked)$(info local=[$(LOCAL_SLOT)] global=[$(GLOBAL_SLOT)]):
