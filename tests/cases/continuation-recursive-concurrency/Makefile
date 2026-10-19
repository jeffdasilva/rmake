all: recurse other;@test -f child && test -f other
recurse:
	+@$(MAKE) -f child.mk --no-print-directory
	@touch recurse
other:
	@touch other.started;i=0;while ! test -f child.started;do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done
	@touch other
