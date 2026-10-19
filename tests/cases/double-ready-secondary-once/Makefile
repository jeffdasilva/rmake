await = i=0;while ! test -f $(1);do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done
.SECONDEXPANSION:
all: left right
left:: $$(file >>expanded,one) leftleaf;@touch left;printf first > trace
left:: $$(file >>expanded,two);@test -f left;printf second >> trace;test "$$(wc -l < expanded)" -eq 2
right: rightleaf
leftleaf:;@touch left.started;$(call await,right.started);touch $@
rightleaf:;@touch right.started;$(call await,left.started);touch $@
