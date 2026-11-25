.SECONDEXPANSION:
colon := :
V := global
lib.a(item.out): private V := parent
READ = $(eval $(1)$(colon) PROBED := $$(info read|$(2)|$$@|$$%|$$*|$$<|$$^|$$+|$$||$$(.SHELLSTATUS)|$$(V)|$$(MARK)))
SET = $(shell exit 17)
ACCEPT = $(info accepted-search|$@|$*|$<)$(eval MARK += accepted)$(SET)
REJECT = $(info rejected-search|$@|$*|$<)$(eval MARK += rejected)$(shell exit 23)
parent-recipe = @printf '%s\n' 'parent|$@|$%|$*|$<|$^|$+|$||$(.SHELLSTATUS)|$(V)|$(MARK)'
child-recipe = @printf '%s\n' 'child|$@|$*|$(.SHELLSTATUS)|$(V)|$(MARK)'
ordinary-child = $(call READ,lib.a(item.out),child)
after = $(call READ,lib.a(item.out),after)
all: lib.a(item.out) after
lib.a(item.out): input input | ordered
after: $$(after) ; @:
(%.out): $$(ACCEPT) probe ; $(parent-recipe)
probe: $$(ordinary-child) ; $(child-recipe)
