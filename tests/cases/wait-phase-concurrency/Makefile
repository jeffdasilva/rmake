all: a b .WAIT c d
a b:;@touch $@.started;i=0;while ! test -f $(if $(filter a,$@),b,a).started;do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done;touch $@
c d:;@test -f a && test -f b;touch $@.started;i=0;while ! test -f $(if $(filter c,$@),d,c).started;do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done;touch $@
