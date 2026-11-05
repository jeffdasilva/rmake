BASE := $(shell /bin/sh -c 'exit 7')
$(info result=[$(call shell,/usr/bin/false)] status=$(.SHELLSTATUS))
$(info outer=[$(.SHELLSTATUS)] $(origin .SHELLSTATUS) $(flavor .SHELLSTATUS))
all:; @:
