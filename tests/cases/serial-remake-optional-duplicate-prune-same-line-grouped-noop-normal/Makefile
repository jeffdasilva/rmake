-include inc.mk inc.mk
all:
	@echo ALL:$(CONTENT)
.PHONY: force order
inc.mk peer.mk &:: force
	@echo FIRST
inc.mk peer.mk &:: force
	@echo SECOND
