await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 250 || exit 8;sleep .01;done
all: left right other;@test "$$(cat count)" = onetwo
left: a
right: b
a b &::
	@printf one >> count;touch group.started;$(call await,other.started)
	@printf two >> count;touch -d @1000000000 a b
other:;@touch other.started;$(call await,group.started);touch other
