$(info child $(origin V):[$(value V)])
.PHONY: child
child: ; +@$(MAKE) -f leaf.mk leaf
