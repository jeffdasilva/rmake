await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 250 || exit 8;sleep .01;done
all: a b
a b &: first;@test -f first && test -f second && touch a b
b: second
first:;@touch first.started;$(call await,second.started);touch first
second:;@touch second.started;$(call await,first.started);touch second
