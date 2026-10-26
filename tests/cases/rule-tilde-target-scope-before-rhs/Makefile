$(shell python3 setup.py)
unexport HOME
HOME=$(shell test -e marker && printf late || printf early)
~/target: X:=$(file >marker,seen)ok
all: early/target
early/target:;@printf "[%s|%s]\n" "$@" "$(X)"
