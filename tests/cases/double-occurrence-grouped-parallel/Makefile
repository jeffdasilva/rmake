await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 250 || exit 8;sleep .01;done
all: left right other;@test "$$(cat count)" = group
left: a
right: b
a b a &:: input;@printf group >> count;touch group.started;$(call await,other.started);touch -d @1000000000 a b
input:;@sleep .02;touch input
other:;@touch other.started;$(call await,group.started);touch other
