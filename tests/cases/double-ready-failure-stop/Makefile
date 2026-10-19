await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done
all: left right
left:: bad;@touch forbidden
left::;@touch forbidden-second
right: slow;@touch forbidden-right
bad:;@$(call await,right.started);exit 7
slow:;@touch right.started;sleep .1;touch completed
