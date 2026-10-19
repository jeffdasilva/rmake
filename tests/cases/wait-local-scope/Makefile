all: other group
other:;@touch started;i=0;while ! test -f after;do i=$$((i+1));test $$i -lt 300 || exit 8;sleep .01;done;touch other
group: before .WAIT after
before:;@touch before
after:;@test -f before;touch after
