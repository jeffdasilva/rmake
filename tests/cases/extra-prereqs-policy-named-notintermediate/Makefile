.NOTINTERMEDIATE:hidden.mid
.EXTRA_PREREQS=hidden.mid
$(shell python3 setup.py)
all:out;@echo all
out:input;@echo built
%.mid:%.src;@echo middle;cp $< $@
