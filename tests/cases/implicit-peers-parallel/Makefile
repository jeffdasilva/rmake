await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 250 || exit 8;sleep .01;done
all: left right other;@test "$$(cat count)" = group
left: x.a
right: x.b
%.a %.b:;@printf group >> count;touch group.started;$(call await,other.started);touch x.a x.b
other:;@touch other.started;$(call await,group.started);touch other
