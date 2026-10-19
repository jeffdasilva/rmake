await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done
all: left right
left: leftleaf;@$(eval FINISHED=yes)test "$(FINISHED)" = yes;touch $@
right: rightleaf;@test -f rightleaf;touch $@
leftleaf:;@touch left.started;$(call await,right.started);touch $@
rightleaf:;@touch right.started;$(call await,left.started);touch $@
