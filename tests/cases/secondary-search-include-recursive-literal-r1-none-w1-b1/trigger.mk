MAKEFLAGS += -I~/incÿ
-include answer.mk
$(info RESULT=[$(RESULT)] DIRS=[$(filter ~/incÿ hdir/incÿ,$(.INCLUDE_DIRS))] RAW=[$(filter -I~/incÿ,$(MAKEFLAGS))])
