Y=before
override MAKEFLAGS = X:=$$(eval\ Y:=nested)value
$(info after=[$(X)]/[$(value X)]/$(origin X)/$(flavor X):[$(Y)]/$(origin Y):[$(MAKEFLAGS)]:$(.SHELLSTATUS))
all: ; @$(info recipe=[$(X)]/[$(value X)]/$(origin X)/$(flavor X):[$(Y)]/$(origin Y):[$(MAKEFLAGS)]:$(.SHELLSTATUS))true
