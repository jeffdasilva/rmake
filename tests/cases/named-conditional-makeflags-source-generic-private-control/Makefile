$(file >trace,READ_START)$(info READ_START)
X := first
all dep: private X ?= second
$(file >>trace,READ_END)$(info READ_END)
all: ; @$(file >>trace,RECIPE)$(info RECIPE):
