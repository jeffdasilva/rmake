%.job: VALUE != /usr/bin/printf '$$(SEED)'
%.job: SEED := pattern
one.job: SEED := explicit
one.job: ; $(info file=[$(VALUE)/$(flavor VALUE)/$(.SHELLSTATUS)])@:
