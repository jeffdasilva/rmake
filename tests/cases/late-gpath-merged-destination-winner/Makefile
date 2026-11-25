VPATH = src
GPATH = src
export .SHELLSTATUS
colon := :
SET = $(shell exit 7)
NAME = $(eval parent$(colon) export ANCHOR_3190 := source)
VIEW = $(info scope=[$@|$*|$<|$^|$(.SHELLSTATUS)|$(origin .SHELLSTATUS)|$(V)|$(origin V)|$(P)|$(A)])
REOPEN = $(eval src/parent$(colon) SEEN := $$(VIEW)$$(shell /usr/bin/python3 exports.py))
DISPLAY = $(eval src/parent$(colon) VIEWED := $$(info env=[$$(SEEN)]))
src/parent: @ := destination-at
src/parent: .SHELLSTATUS := destination-status
parent: input
	@$(SET)$(NAME)touch src/parent
after:
	@$(REOPEN)$(DISPLAY):
