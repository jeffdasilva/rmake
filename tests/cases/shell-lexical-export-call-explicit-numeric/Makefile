export 1 := global
F = $(info local=$(1) child=[$(shell /usr/bin/printenv 1)] status=$(.SHELLSTATUS))
all:;@$(call F,argument)$(info after=[$(shell /usr/bin/printenv 1)] status=$(.SHELLSTATUS)):
