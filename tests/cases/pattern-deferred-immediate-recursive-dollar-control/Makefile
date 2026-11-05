SEED := before
%.out: VALUE :::= $(SEED)-$$literal
SEED := changed
$(info read=[$(SEED)])
item.out:
	@printf 'value=[%s] raw=[%s] seed=[%s] extra=[%s]\n' '$(VALUE)' '$(value VALUE)' '$(SEED)' '$(EXTRA)'
