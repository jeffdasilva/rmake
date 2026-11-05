SEED := before
%.out: VALUE :::= $(eval SEED := changed)$(SEED)-$$literal
$(info read=[$(SEED)])
item.out:
	@printf 'value=[%s] raw=[%s] seed=[%s] extra=[%s]\n' '$(VALUE)' '$(value VALUE)' '$(SEED)' '$(EXTRA)'
