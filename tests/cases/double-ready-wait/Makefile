await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done
all: left right
left:: leftleaf .WAIT after;@touch left;printf first > trace
left::;@test -f left;printf second >> trace
after:;@test -f leftleaf;touch $@
right: rightleaf
leftleaf:;@touch left.started;$(call await,right.started);touch $@
rightleaf:;@touch right.started;$(call await,left.started);touch $@
