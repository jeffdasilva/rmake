SOURCE = lib.a
GPATH = src
VPATH = src
colon := :
UPDATE = $(eval $(SOURCE)$(colon) override .SHELLSTATUS = inherited-$$(V))
READ_SOURCE = $(eval $(SOURCE)$(colon) Q := $$(info raw=[$$@|$$(.SHELLSTATUS)|$$(origin .SHELLSTATUS)]))
READ_DEST = $(eval src/$(SOURCE)$(colon) Q := $$(info searched=[$$(.SHELLSTATUS)|$$(origin .SHELLSTATUS)]))
$(SOURCE): V := source
src/$(SOURCE): V := destination
$(SOURCE):
	@$(shell exit 7)cp seed $(SOURCE)
move:
	@$(UPDATE)mv $(SOURCE) src/$(SOURCE)
after:
	@$(READ_SOURCE)$(READ_DEST):
