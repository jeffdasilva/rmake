VPATH = src
GPATH = src
export .SHELLSTATUS
colon := :
WRITE = $(shell exit 7)
NAME = $(eval parent$(colon) export ANCHOR_3190 := named)
READING = $(info retained=[$@|$*|$<|$^|$(.SHELLSTATUS)|$(origin .SHELLSTATUS)|$(V)])
REOPEN = $(eval src/parent$(colon) SEEN := $$(READING)$$(shell /usr/bin/python3 env_order.py))
DISPLAY = $(eval src/parent$(colon) DISPLAY := $$(info env=[$$(SEEN)]))
parent: input
	@$(WRITE)$(NAME)touch src/parent
after:
	@$(REOPEN)$(DISPLAY):
