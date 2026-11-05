KEY := global
caller: KEY := caller-own
.PHONY: all caller chosen
all: caller chosen
caller: ; $(eval chosen: VALUE != /usr/bin/printf '$$(KEY)')$(info caller=[$(KEY)/$(.SHELLSTATUS)])@:
chosen: ; $(info chosen=[$(VALUE)/$(KEY)/$(.SHELLSTATUS)])@:
