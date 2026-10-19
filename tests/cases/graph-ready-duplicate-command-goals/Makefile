await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done
left: leaf
right:;@touch right.started
leaf:;@$(call await,right.started);touch leaf
