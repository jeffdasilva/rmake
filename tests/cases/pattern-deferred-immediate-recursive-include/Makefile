SEED := before
%.out: VALUE :::= $(eval include defs.mk)$(SEED)
$(info read=[$(SEED)])
item.out:
	@printf 'value=[%s] raw=[%s] seed=[%s] extra=[%s]\n' '$(VALUE)' '$(value VALUE)' '$(SEED)' '$(EXTRA)'
	@test "$$(cat seen)" = "$$(printf 'anchor\nincluded\n')"
