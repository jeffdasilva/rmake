$(file >seen,anchor)
unexport HOME Z RESULT
HOME = $(file >>seen,HOME)hdir
RESULT := missing
undefine Z
GNUMAKEFLAGS = -I~/inc -- Z:=good
.PHONY: all
all:;@$(eval -include answer.mk)$(info RESULT=[$(RESULT)] Z=[$(Z)])true
