await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done
all: left right
left: recurse
right: rightleaf
recurse:; +@$(MAKE) -f child.mk --no-print-directory
rightleaf:;@touch right.started;$(call await,left.started);touch $@
