MAKEFLAGS += -I~/inc
MAKEFLAGS += -s
MAKEFLAGS += -s
-include answer.mk
$(info RESULT=[$(RESULT)] DIRS=[$(filter ~/inc hdir/inc,$(.INCLUDE_DIRS))] RAW=[$(filter -I~/inc,$(MAKEFLAGS))])
