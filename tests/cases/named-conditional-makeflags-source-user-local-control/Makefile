$(file >trace,READ_START)$(info READ_START)
LOCAL = --keep-going
all: MAKEFLAGS = $(LOCAL)
all dep: MAKEFLAGS ?= --silent
$(file >>trace,READ_END)$(info READ_END)
all: ; @$(file >>trace,RECIPE)$(info RECIPE):
