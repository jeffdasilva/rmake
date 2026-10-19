all: a .WAIT recurse
a:;@touch a
recurse:; +@$(MAKE) -f child.mk --no-print-directory
