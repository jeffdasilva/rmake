-include opt.mk
include req.mk
req-mid opt-mid: leaf
	@echo MIDDLE
req.mk: req-mid
	@echo REQUIRED
opt.mk: opt-mid
	@echo OPTIONAL
all:
	@echo ALL
leaf:
	@echo LEAF; exit 7
