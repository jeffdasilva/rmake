$(info reader=[$(shell /bin/echo OPEN)] status=[$(.SHELLSTATUS)])
all: result .WAIT tail
result:
	@/bin/echo OUT-first
	@/bin/echo OUT-last
	@/usr/bin/touch result
tail: ; @/bin/echo TAIL
