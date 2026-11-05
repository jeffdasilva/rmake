ifeq ($(MAKE_RESTARTS),)
BEFORE := $(shell python3 -B witness.py before)
ABSENT := $(shell ./absent)
ABSENT_STATUS := $(.SHELLSTATUS)
AFTER_ONE := $(shell python3 -B witness.py after-one)
DENIED != ./denied
DENIED_STATUS := $(.SHELLSTATUS)
AFTER_TWO := $(shell python3 -B witness.py after-two)
SUCCESS_STATUS := $(.SHELLSTATUS)
$(file >statuses,[$(ABSENT)]:$(ABSENT_STATUS) [$(DENIED)]:$(DENIED_STATUS) [$(AFTER_TWO)]:$(SUCCESS_STATUS))
else
RESTARTED := $(shell python3 -B witness.py restarted)
endif

.PHONY: all
all:
	@printf 'restart=%s\n' '$(MAKE_RESTARTS)'

include generated.mk
generated.mk:
	@printf '%s\n' 'REBUILT := yes' >$@
