await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done
left: leaf;@$(error stop)
right: slow;@touch forbidden
leaf:;@sleep .03;touch leaf
slow:;@sleep .1;touch slow
