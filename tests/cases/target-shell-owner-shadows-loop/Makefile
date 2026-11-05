KEY := permanent
chosen: KEY := owner
$(foreach KEY,root-loop,$(eval chosen: VALUE != /usr/bin/printf '$$(KEY)')$(info during=[$(KEY)]))
chosen: ; $(info chosen=[$(VALUE)/$(KEY)/$(.SHELLSTATUS)])@:
