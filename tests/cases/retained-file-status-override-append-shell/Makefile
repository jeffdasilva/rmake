export .SHELLSTATUS
colon := :
EARLY = $(eval parent$(colon) export ANCHOR_3190 := before)
LATE = $(eval parent$(colon) export ANCHOR_4413 := after)
OLD = $(shell exit 23)
NEXT = $(shell exit 29)
VIEW = $(info status=[$@|$(.SHELLSTATUS)|$(origin .SHELLSTATUS)|$(flavor .SHELLSTATUS)|$(value .SHELLSTATUS)])
READ = $(eval parent$(colon) VIEWED := $$(VIEW))
CAPTURE = $(eval parent$(colon) CAPTURED := $$(shell /usr/bin/python3 exports.py))
REPORT = $(eval parent$(colon) SHOWN := $$(info env=[$$(CAPTURED)]))
parent: V := local
CHANGE = $(eval parent$(colon) override .SHELLSTATUS += $$(shell exit 31)accepted)
parent: input
	@$(EARLY)$(OLD)$(LATE)$(NEXT):
after:
	@$(CHANGE)$(READ)$(CAPTURE)$(REPORT):
