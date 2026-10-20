await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 250 || exit 8;sleep .01;done
all: left other;@test "$$(cat count)" = twotwo
left: a
a a :: input;@printf two >> count;touch group.started;$(call await,other.started);touch a
input:;@sleep .02;touch input
other:;@touch other.started;$(call await,group.started);touch other
