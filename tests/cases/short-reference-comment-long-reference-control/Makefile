HASH := \#
SLASH := $(subst x,\,x)
define $(HASH)
H
endef
define $(SLASH)
S
endef
X := x
V = $(subst x,x,$\#tail) # discard
$(info raw=[$(value V)] expanded=[$(V)])
all: ; @:
