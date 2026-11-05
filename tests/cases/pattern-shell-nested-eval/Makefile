SEED := global
%.job: SEED := pattern
%.job: VALUE != $(eval PUBLISHED := $(SEED))/usr/bin/printf '$(PUBLISHED)'
$(info reader=[$(PUBLISHED)/$(SEED)])
one.job: ; $(info file=[$(VALUE)/$(SEED)/$(PUBLISHED)/$(.SHELLSTATUS)])@:
