$(shell touch -d @1600000000 cached; touch -d @1600000001 newer)
gate: ; @$(info gate):
enable: ; @$(eval override MAKEFLAGS += -k)$(info enable):
disable: ; @$(eval override MAKEFLAGS += -S)$(info disable):
after: ; @$(info after):
cached:: newer ; @$(info head):
cached:: ; @$(eval override MAKEFLAGS += -S)$(info tail-disable):
