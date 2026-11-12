'$(info capture=[$(shell /bin/echo OPEN)] status=[$(.SHELLSTATUS)])
all: ordinary .WAIT recursive
ordinary: ; @/usr/bin/python3 probe.py ordinary
recursive: ; +@/usr/bin/python3 probe.py recursive
