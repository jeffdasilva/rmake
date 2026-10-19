all: left right
left: bad;@touch forbidden-left
right: slow;@touch forbidden-right
bad:;@i=0;while ! test -f started;do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done;exit 7
slow:
	@touch started;sleep .1;touch completed
	@touch continuation-completed
