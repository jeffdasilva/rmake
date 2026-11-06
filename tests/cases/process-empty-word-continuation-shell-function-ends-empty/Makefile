empty :=
space := $(empty) $(empty)
backslash := \$(empty)
define linefeed


endef
$(file >command.txt,./capture top ''$(backslash)$(linefeed)$(space)$(space)$(space)$(space))
$(info result=[$(shell $(file <command.txt))])
all: ;
