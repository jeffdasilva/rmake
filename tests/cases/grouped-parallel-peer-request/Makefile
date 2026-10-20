await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 250 || exit 8;sleep .01;done
all: left right other;@test "$$(cat count)" = group
left: a
right: b
a b &:;@printf group >> count;touch group.started;$(call await,other.started);touch a b
other:;@touch other.started;$(call await,group.started);touch other
