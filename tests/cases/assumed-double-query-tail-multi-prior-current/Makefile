$(shell touch -d @1600000000 cached primer; touch -d @1600000001 newer; touch -d @1600000000 current)
.DEFAULT_GOAL := cached
cached:: primer
	@$(error current declared branch must not expand)
.SECONDEXPANSION:
ca%: $$(if $$(wildcard chosen),$$(info search-new)newer,$$(info search-old)$$(file >chosen,yes)primer)
	@$(info synthetic-query):
current: ; @$(error current goal must not expand)
after: ; +@printf 'after\n'
