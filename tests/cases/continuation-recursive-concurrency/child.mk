child:;@touch child.started;i=0;while ! test -f other.started;do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done;touch child
