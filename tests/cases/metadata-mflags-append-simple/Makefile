MFLAGS := file
$(info before MFLAGS=[$(MFLAGS)]/[$(value MFLAGS)]/$(origin MFLAGS)/$(flavor MFLAGS) MAKEFLAGS=[$(MAKEFLAGS)])
MAKEFLAGS += -k
$(info after MFLAGS=[$(MFLAGS)]/[$(value MFLAGS)]/$(origin MFLAGS)/$(flavor MFLAGS) MAKEFLAGS=[$(MAKEFLAGS)])
all: ; @$(info recipe MFLAGS=[$(MFLAGS)]/[$(value MFLAGS)]/$(origin MFLAGS)/$(flavor MFLAGS) MAKEFLAGS=[$(MAKEFLAGS)])true
