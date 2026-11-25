SOURCE = bundle
colon := :
UPDATE = $(eval $(SOURCE)$(colon) override .SHELLSTATUS = inherited-$$(V))
READ_SOURCE = $(eval $(SOURCE)$(colon) Q := $$(info raw=[$$@|$$(.SHELLSTATUS)|$$(origin .SHELLSTATUS)|$$(flavor .SHELLSTATUS)|$$(value .SHELLSTATUS)|$$(V)]))
READ_DEST = $(eval src/$(SOURCE)$(colon) Q := $$(info searched=[$$@|$$(.SHELLSTATUS)|$$(origin .SHELLSTATUS)|$$(flavor .SHELLSTATUS)|$$(value .SHELLSTATUS)|$$(V)]))
$(SOURCE): V := source
$(SOURCE):
	@$(shell exit 7)cp seed $(SOURCE)
move:
	@$(UPDATE)$(eval vpath % src)mv $(SOURCE) src/$(SOURCE)
REBOUND = $(eval src/$(SOURCE)$(colon) override .SHELLSTATUS := rebound)
after:
	@$(READ_DEST)$(REBOUND)$(READ_DEST)$(READ_SOURCE):
