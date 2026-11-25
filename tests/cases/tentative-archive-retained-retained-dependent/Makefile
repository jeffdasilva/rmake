SOURCE = bundle
colon := :
UPDATE = $(eval $(SOURCE)$(colon) override .SHELLSTATUS = inherited-$$(V))
READ_SOURCE = $(eval $(SOURCE)$(colon) Q := $$(info raw=[$$@|$$(.SHELLSTATUS)|$$(origin .SHELLSTATUS)|$$(flavor .SHELLSTATUS)|$$(value .SHELLSTATUS)|$$(V)]))
READ_DEST = $(eval src/$(SOURCE)$(colon) Q := $$(info searched=[$$@|$$(.SHELLSTATUS)|$$(origin .SHELLSTATUS)|$$(flavor .SHELLSTATUS)|$$(value .SHELLSTATUS)|$$(V)]))
$(SOURCE): V := source
$(SOURCE): piece
	@$(shell exit 7)cp seed $(SOURCE)
move:
	@$(UPDATE)$(eval vpath % src)mv $(SOURCE) src/$(SOURCE)
READ_CHILD = $(eval piece$(colon) X := $$(info child=[$$@|$$(V)|$$(.SHELLSTATUS)]))
after:
	@$(READ_DEST)$(READ_CHILD)$(READ_SOURCE):
piece: ; @:
