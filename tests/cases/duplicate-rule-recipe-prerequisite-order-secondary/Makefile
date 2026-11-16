.SECONDEXPANSION:
all: old $$(info old=[$$@][$$+][$$|]) | tail
all all: first $$(info current=[$$@][$$<][$$+][$$|]) | order
	@printf '%s\n' 'recipe=[$@][$<][$^][$+][$|]'
first old order tail: ;
