SEMICOLON := ;
SLASH := $(subst x,\,x)
define $(SEMICOLON)
M
endef
define $(SLASH)
S
endef
X := x
all: item$\;tail # comment ; $(error not-inline)
	@printf '%s\n' '$^'
