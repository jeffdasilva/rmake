all: left right;@test -f left && test -f right
left right:
	@touch $@.started;i=0;while ! test -f $(if $(filter left,$@),right,left).started;do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done
	@printf done > $@
