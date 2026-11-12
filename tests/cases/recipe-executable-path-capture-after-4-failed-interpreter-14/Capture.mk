export PATH := .:/usr/bin:/bin
$(info bad=[$(shell runner)] status=[$(.SHELLSTATUS)])
$(info bad=[$(shell runner)] status=[$(.SHELLSTATUS)])
$(info bad=[$(shell runner)] status=[$(.SHELLSTATUS)])
$(info bad=[$(shell runner)] status=[$(.SHELLSTATUS)])
$(info valid=[$(shell /bin/echo RAN)] status=[$(.SHELLSTATUS)])
all: ; @:
