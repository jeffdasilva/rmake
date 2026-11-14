$(info reader=[$(shell /bin/echo OPEN)] status=[$(.SHELLSTATUS)])
all: result .WAIT tail
result:
	@/bin/echo OUT-first
	@/bin/echo ERR-first >&2
	@/bin/echo OUT-last
	@/bin/echo ERR-last >&2
	@/usr/bin/touch result
tail: ; @/bin/echo TAIL
