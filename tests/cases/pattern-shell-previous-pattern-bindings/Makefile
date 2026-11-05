SEED := global
%.job: SEED := pattern
%.job: FIRST != /bin/sh -c 'printf first; exit 4'
special.%.job: SECOND != /usr/bin/printf '$(SEED)/$(FIRST)/$(.SHELLSTATUS)'
special.one.job: SEED := explicit
special.one.job: ; $(info file=[$(SECOND)/$(SEED)/$(.SHELLSTATUS)])@:
