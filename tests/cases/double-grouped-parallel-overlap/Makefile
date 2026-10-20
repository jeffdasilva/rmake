await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 250 || exit 8;sleep .01;done
all: left right other;@test "$$(cat first.count)" = first;test "$$(cat second.count)" = second
left: a
right: c
a b &::;@printf first >> first.count;touch first.started;$(call await,second.started);touch -d @1000000000 a b
b c &::;@printf second >> second.count;touch second.started;$(call await,first.started);touch -d @1000000000 b c
other:;@touch other
