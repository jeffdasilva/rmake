LIVE := global
parent: child
	@$(info parent=[$(LIVE)]):
child:;@$(eval parent: private LIVE := hidden)$(info child=[$(LIVE)]):
