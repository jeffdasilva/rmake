A != /usr/bin/printf value
$(info a=[$(A)] status=$(.SHELLSTATUS))
B != /bin/sh -c 'exit 4'
$(info b=[$(B)] blank=[$(shell )] status=$(.SHELLSTATUS))
$(info outer=[$(.SHELLSTATUS)] $(origin .SHELLSTATUS) $(flavor .SHELLSTATUS))
all:; @:
