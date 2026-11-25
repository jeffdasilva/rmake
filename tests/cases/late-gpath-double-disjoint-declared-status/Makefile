VPATH = src
GPATH = src
.SECONDEXPANSION:
colon := :
HEAD = $(shell exit 7)
LATER = $(shell exit 13)
WRITE_HEAD = $(eval $(DEST)$(colon) MARKER := $$(shell exit 19))
ADD_NAME = $(eval $(DEST)$(colon) AFTER := later-value)
names = $(SOURCE_ONLY)|$(DESTINATION_ONLY)|$(PATTERN_ONLY)|$(AFTER)|$(BEFORE)
VIEW = $(info $(MARK)[$@|$*|$<|$^|$(.SHELLSTATUS)|$(names)])
READ = $(eval $(DEST)$(colon) READ_VALUE := $$(VIEW))
TRACE = $(info recipe=[$@|$*|$<|$^|$(.SHELLSTATUS)|$(names)])
parent: SOURCE_ONLY := source
parent: override .SHELLSTATUS := declared
src/parent: DESTINATION_ONLY := destination
DEST = src/parent
MARK = canonical=
parent:: input
	@$(TRACE)touch -r input src/parent
parent:: other
	@$(LATER)$(TRACE):
after:;@$(READ)$(WRITE_HEAD)$(READ):
