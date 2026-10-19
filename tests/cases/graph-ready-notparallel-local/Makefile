await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done
.NOTPARALLEL: left
all: left right
left: leftleaf after
after:;@test -f leftleaf;touch $@
right: rightleaf
leftleaf:;@touch left.started;$(call await,right.started);touch $@
rightleaf:;@touch right.started;$(call await,left.started);touch $@
