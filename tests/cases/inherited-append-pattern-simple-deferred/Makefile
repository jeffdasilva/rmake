X := global
KEY := reader
%.task: X += $(eval MARK := $(KEY))tail-$(KEY)
$(info reading=[$(MARK)/$(X)])
one.task: KEY := target
one.task: ; $(info pre=[$(MARK)] target=[$(X)/$(MARK)/$(flavor X)])@:
