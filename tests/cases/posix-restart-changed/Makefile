.POSIX:
$(shell test ! -f made.mk || touch -t 200001010000 made.mk)
include made.mk
$(info read flags=[$(.SHELLFLAGS)] restarts=[$(MAKE_RESTARTS)])
all: ; @printf done\n
made.mk: ; @printf ".POSIX:\nREADY=yes\n" > $@
