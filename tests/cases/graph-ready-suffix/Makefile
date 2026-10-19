await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done
.SUFFIXES: .in .out
all: left.out right.out
.in.out:;@cp $< $@
left.in:;@touch left.started;$(call await,right.started);touch $@
right.in:;@touch right.started;$(call await,left.started);touch $@
