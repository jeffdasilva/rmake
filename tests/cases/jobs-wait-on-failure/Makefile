all: bad slow
bad:;@i=0;while ! test -f started; do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done;exit 7
slow:;@touch started;sleep .05;touch completed
