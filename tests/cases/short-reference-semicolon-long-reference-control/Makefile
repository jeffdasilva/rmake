SEMICOLON := ;
SLASH := $(subst x,\,x)
define $(SEMICOLON)
M
endef
define $(SLASH)
S
endef
X := x
all: item$(subst t,t,$\;tail) ; @printf '%s\n' '$^'
