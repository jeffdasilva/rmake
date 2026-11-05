define PATCH
chosen: X := changed
MARK := $$(X)
endef
X = $(eval $(subst $$$$,$$,$(value PATCH)))outer
chosen: X += tail
chosen: ; $(info first=[$(X)] second=[$(X)] marker=[$(MARK)])@:
