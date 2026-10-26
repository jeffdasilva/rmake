$(shell python3 setup.py)
unexport HOME
HOME=$(error unused)
include identity.mk
all: ~$(ACCOUNT);@python3 check.py '$<'
