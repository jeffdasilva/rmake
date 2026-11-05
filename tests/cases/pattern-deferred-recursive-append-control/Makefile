SEED := before
%.out: VALUE = head
%.out: VALUE += $(eval SEED := updated)$(SEED)
$(info read=[$(SEED)])
item.out:
	@printf 'value=[%s] raw=[%s] seed=[%s] extra=[%s]\n' '$(VALUE)' '$(value VALUE)' '$(SEED)' '$(EXTRA)'
