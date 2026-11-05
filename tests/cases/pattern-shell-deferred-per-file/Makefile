SEED := early
%.job: VALUE != $(file >>seen,activate=$(SEED) auto=[$@])/usr/bin/printf '$(SEED)'
SEED := late
$(file >>seen,reader-done)
.PHONY: all alpha.job beta.job
all: alpha.job beta.job alpha.job
alpha.job: SEED := explicit
alpha.job beta.job: ; $(info $@=[$(VALUE)/$(SEED)/$(flavor VALUE)/$(.SHELLSTATUS)])@:
