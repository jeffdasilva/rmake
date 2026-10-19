.DEFAULT_GOAL:=all
define T
ifdef V
X=yes
endif
endef
$(foreach V,a,$(eval $(value T)))
$(info $(X))
all:;@:
