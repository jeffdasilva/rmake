export 2 := global
INNER = $(info inner=[$(shell /usr/bin/printenv 2)] status=$(.SHELLSTATUS))
OUTER = $(info outer=[$(shell /usr/bin/printenv 2)] status=$(.SHELLSTATUS))$(call INNER,one)$(info restored=[$(shell /usr/bin/printenv 2)] status=$(.SHELLSTATUS))
all:;@$(call OUTER,one,two):
