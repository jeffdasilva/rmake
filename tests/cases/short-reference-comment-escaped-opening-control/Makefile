HASH := \#
SLASH := $(subst x,\,x)
define $(HASH)
H
endef
define $(SLASH)
S
endef
X := x
V = $$(x$\#tail) # discard
$(info raw=[$(value V)] expanded=[$(V)])
all: ; @:
