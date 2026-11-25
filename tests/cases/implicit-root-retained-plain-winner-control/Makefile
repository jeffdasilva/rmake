.SECONDEXPANSION:
colon := :
V := global
item.out: private V := parent
READ = $(eval $(1)$(colon) PROBED := $$(info read|$(2)|$$@|$$%|$$*|$$<|$$^|$$+|$$||$$(.SHELLSTATUS)|$$(V)|$$(MARK)))
SET = $(shell exit 17)
ACCEPT = $(info accepted-search|$@|$*|$<)$(eval MARK += accepted)$(SET)
REJECT = $(info rejected-search|$@|$*|$<)$(eval MARK += rejected)$(shell exit 23)
parent-recipe = @printf '%s\n' 'parent|$@|$%|$*|$<|$^|$+|$||$(.SHELLSTATUS)|$(V)|$(MARK)'
child-recipe = @printf '%s\n' 'child|$@|$*|$(.SHELLSTATUS)|$(V)|$(MARK)'
ordinary-child = $(call READ,item.out,child)
after = $(call READ,item.out,after)
all: item.out after
item.out: input input | ordered
after: $$(after) ; @:
%.out: probe ; $(parent-recipe)
probe: $$(ordinary-child) ; $(child-recipe)
