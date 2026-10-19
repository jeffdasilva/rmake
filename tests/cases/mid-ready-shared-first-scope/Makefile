await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done
prepare := $(shell touch -d @1000000000 old;touch -d @1000000010 final)
.INTERMEDIATE: middle
prepare_more := $(shell touch -d @1000000010 finalA finalB)
all: finalA finalB right;@test "$$(cat count)" = first
finalA: WHO=first
finalB: WHO=second
finalA finalB: middle;@touch $@
middle: leftleaf;@printf "$(WHO)" >> count;touch $@
right: rightleaf
leftleaf:;@touch left.started;$(call await,right.started);touch $@
rightleaf:;@touch right.started;$(call await,left.started);touch $@
