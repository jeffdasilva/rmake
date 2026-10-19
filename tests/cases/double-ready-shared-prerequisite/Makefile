await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done
all: left right;@test "$$(cat count)" = once
left:: shared;@touch first
left:: shared;@test -f first;touch second
right: rightleaf
shared:;@printf once >> count;touch left.started;$(call await,right.started);touch $@
rightleaf:;@touch right.started;$(call await,left.started);touch $@
