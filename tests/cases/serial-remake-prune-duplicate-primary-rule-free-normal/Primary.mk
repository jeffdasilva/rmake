ifeq ($(primary_read),)
primary_read := 1
$(info READ:$(MAKE_RESTARTS))
all:
	@echo ALL:$(CONTENT)
endif
