await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done
all: left right
left: bad
right: good
bad:; -@touch left.started;$(call await,right.started);exit 7
good:;@touch right.started;$(call await,left.started);touch $@
