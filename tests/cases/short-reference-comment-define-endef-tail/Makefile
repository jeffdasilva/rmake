HASH := \#
SLASH := $(subst x,\,x)
define $(HASH)
H
endef
define $(SLASH)
S
endef
X := x
define V
one
endef $\#tail
$(info raw=[$(value V)] expanded=[$(V)])
all: ; @:
