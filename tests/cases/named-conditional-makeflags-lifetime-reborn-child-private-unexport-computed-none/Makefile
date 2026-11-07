undefine MAKEFLAGS
MAKEFLAGS = $(file >>reads,reborn)-k
all: child sibling
child: private unexport $(file >>names,expanded)MAKEFLAGS ?= $(file >>rhs,expanded)-s
child:
	@$(info child=[$(origin MAKEFLAGS)]/[$(MAKEFLAGS)]/[$(MFLAGS)])printf 'child=[%s]/[%s]\n' "$$(printenv MAKEFLAGS)" "$$(printenv MFLAGS)" >> effects
sibling:
	@$(info sibling=[$(origin MAKEFLAGS)]/[$(MAKEFLAGS)]/[$(MFLAGS)])printf 'sibling=[%s]/[%s]\n' "$$(printenv MAKEFLAGS)" "$$(printenv MFLAGS)" >> effects
all:
	@$(info all=[$(origin MAKEFLAGS)]/[$(MAKEFLAGS)]/[$(MFLAGS)])printf 'all=[%s]/[%s]\n' "$$(printenv MAKEFLAGS)" "$$(printenv MFLAGS)" >> effects
after:
	@$(info after=[$(origin MAKEFLAGS)]/[$(MAKEFLAGS)]/[$(MFLAGS)])printf 'after=[%s]/[%s]\n' "$$(printenv MAKEFLAGS)" "$$(printenv MFLAGS)" >> effects
