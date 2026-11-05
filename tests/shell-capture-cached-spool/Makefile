load ./spool_slots.so
.PHONY: all
all:
	@$(info spool-before)$(spool_close $(LAYOUT))$(eval CAPTURE := $$(shell $(CAPTURE_COMMAND)))$(eval STATUS := $$(.SHELLSTATUS))$(eval LOW := $$(spool_restore ))$(file >>seen,status=[$(STATUS)]|slots=[$(LOW)]|result=[$(CAPTURE)])$(info status=[$(STATUS)]|slots=[$(LOW)]|result=[$(CAPTURE)]):
