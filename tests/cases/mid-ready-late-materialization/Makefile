await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done
prepare := $(shell touch -d @1000000000 old;touch -d @1000000010 final)
.INTERMEDIATE: middle
all: final other
final: middle trigger;@test -f middle;touch $@
middle: leftleaf;@$(call await,other.done);touch $@
leftleaf:;@touch -r old $@
trigger:;@sleep .03;touch $@
other: otherleaf;@touch other.done
otherleaf:;@sleep .05;touch $@
