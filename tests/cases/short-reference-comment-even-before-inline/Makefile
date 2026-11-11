HASH := \#
SLASH := $(subst x,\,x)
define $(HASH)
H
endef
define $(SLASH)
S
endef
X := x
all: item$\\# ; $(error not-inline)
	@printf "%s\n" "$^"
