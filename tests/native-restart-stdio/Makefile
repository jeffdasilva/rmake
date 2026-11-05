.DEFAULT_GOAL := all
load ./stdio_probe.so
$(file >>seen,read-$(MAKE_RESTARTS))
-include generated.mk
generated.mk:
	@$(file >generated.mk,READY := yes)$(stage_stdio $(MODE))
all:
	@$(file >>seen,goal-$(MAKE_RESTARTS))printf 'ready=%s restart=%s\n' '$(READY)' '$(MAKE_RESTARTS)'
