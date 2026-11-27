$(file >trace,READ_START)$(info READ_START)
all: private MAKEFLAGS ?= --silent
$(file >>trace,READ_END)$(info READ_END)
all dep &: ; @$(file >>trace,RECIPE)$(info RECIPE):
