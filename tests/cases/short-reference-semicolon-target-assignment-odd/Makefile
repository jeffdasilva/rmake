SEMICOLON := ;
SLASH := $(subst x,\,x)
define $(SEMICOLON)
M
endef
define $(SLASH)
S
endef
X := x
all: V = $\;tail
all: ; @printf "raw=[%s] expanded=[%s]\n" "$(value V)" "$(V)"
