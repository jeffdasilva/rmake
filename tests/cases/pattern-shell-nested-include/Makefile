SEED := global
%.job: SEED := pattern
%.job: VALUE != $(eval include command.mk)/usr/bin/printf '$(COMMAND)'
$(file >>seen,reader)
one.job: ; $(info file=[$(VALUE)/$(COMMAND)/$(SEED)])@:
