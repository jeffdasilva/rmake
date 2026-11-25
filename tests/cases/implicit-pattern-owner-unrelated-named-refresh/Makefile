.SECONDEXPANSION:
colon := :
%.mid: export V = fixed
%.out: %.mid %.tmp %.q ; @:
%.tmp: %.mid %.mid ; @:
%.q: $$(eval other$$(colon) export X = x) %.in ; @:
%.mid: $$(eval SEEN += x) $$(shell exit $$(words $$(SEEN))) %.in
	@printf '%s|%s|%s|%s|%s\n' '$(V)' '$(X)' "$$V" "$$X" '$(.SHELLSTATUS)'
