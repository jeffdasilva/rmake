await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done
prepare := $(shell touch -d @1000000000 old;touch -d @1000000010 final)
.INTERMEDIATE: middle
all: group right
group: final
final: middle;@test -f middle;touch $@
middle: leftleaf;@touch $@
right: rightleaf
leftleaf:;@touch left.started;$(call await,right.started);touch $@
rightleaf:;@touch right.started;$(call await,left.started);touch $@
