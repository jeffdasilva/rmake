HASH := \#
SLASH := $(subst x,\,x)
define $(HASH)
H
endef
define $(SLASH)
S
endef
X := x
ifeq ($\#tail,Htail)
V = chosen
else
V = wrong
endif
$(info raw=[$(value V)] expanded=[$(V)])
all: ; @:
