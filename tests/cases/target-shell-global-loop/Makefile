KEY := permanent
$(foreach KEY,root-loop,$(eval chosen: VALUE != /usr/bin/printf '$$(KEY)')$(info during=[$(KEY)/$(.SHELLSTATUS)]))
$(info after=[$(KEY)/$(.SHELLSTATUS)])
chosen: ; $(info chosen=[$(VALUE)/$(KEY)/$(.SHELLSTATUS)])@:
