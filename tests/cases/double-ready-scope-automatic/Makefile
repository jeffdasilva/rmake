await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done
all: left right
left: WHO=left
left:: leftleaf;@touch left;printf "%s\n" "$(WHO):$@:$<:$^:$+" > trace
left::;@test -f left;printf "%s\n" "$(WHO):$@:$<:$^:$+" >> trace
right: rightleaf
leftleaf:;@touch left.started;$(call await,right.started);touch $@
rightleaf:;@touch right.started;$(call await,left.started);touch $@
