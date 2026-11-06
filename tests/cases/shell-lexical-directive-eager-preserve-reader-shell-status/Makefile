LEX := global
define DECL
owned: CAP := $(eval export LEX)$(shell /bin/sh -c 'exit 7')$(.SHELLSTATUS)
endef
$(foreach LEX,local,$(shell /bin/sh -c 'exit 3')$(eval $(value DECL))$(info outer-status=$(.SHELLSTATUS) lex=$(shell printf "[%s]" "$$LEX")))
owned:;@$(info cap=$(CAP)):
