$(file >seen,anchor)
unexport HOME Z RESULT
HOME = $(file >>seen,HOME)hdir
RESULT := missing
GNUMAKEFLAGS = -I~/inc -- -f~/MISSING
.PHONY: all
all:;@$(eval -include answer.mk)$(info RESULT=[$(RESULT)] Z=[$(Z)])true
