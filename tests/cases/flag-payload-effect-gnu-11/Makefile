Y=before
GNUMAKEFLAGS = X:=$$(error\ payload)
$(info after=[$(X)]/[$(value X)]/$(origin X)/$(flavor X):[$(Y)]/$(origin Y):[$(MAKEFLAGS)]:$(.SHELLSTATUS))
all: ; @$(info recipe=[$(X)]/[$(value X)]/$(origin X)/$(flavor X):[$(Y)]/$(origin Y):[$(MAKEFLAGS)]:$(.SHELLSTATUS))true
