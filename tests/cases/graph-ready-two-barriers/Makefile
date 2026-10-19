await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done
all: left right
left: leftleaf .WAIT leftafter
right: rightleaf .WAIT rightafter
leftafter:;@test -f leftleaf;touch left.after;$(call await,right.after);touch $@
rightafter:;@test -f rightleaf;touch right.after;$(call await,left.after);touch $@
leftleaf:;@touch left.started;$(call await,right.started);touch $@
rightleaf:;@touch right.started;$(call await,left.started);touch $@
