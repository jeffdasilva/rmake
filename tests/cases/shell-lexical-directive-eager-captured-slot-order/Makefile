export LEX_024 := global-a
export LEX_034 := global-b
define DECL
owned: CAP := $(filter LEX_024=% LEX_034=%,$(shell /usr/bin/env))
endef
$(let LEX_034 LEX_024,local-b local-a,$(eval $(value DECL)))
owned:;@$(info cap=$(CAP)):
