export PATH := .:/usr/bin:/bin
$(info plain=[$(shell plain)] status=[$(.SHELLSTATUS)])
$(info valid=[$(shell /bin/echo RAN)] status=[$(.SHELLSTATUS)])
all: ; @:
