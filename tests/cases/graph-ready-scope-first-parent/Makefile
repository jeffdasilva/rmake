await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done
all: left right other;@test "$$(cat count)" = left
left: WHO=left
right: WHO=right
left right: shared
shared:;@printf "$(WHO)" >> count;touch shared.started;$(call await,other.started);touch $@
other:;@touch other.started;$(call await,shared.started);touch $@
