override MFLAGS = fixed
$(info before MFLAGS=[$(MFLAGS)]/[$(value MFLAGS)]/$(origin MFLAGS)/$(flavor MFLAGS) MAKEFLAGS=[$(MAKEFLAGS)])
override MAKEFLAGS = -s
$(info after MFLAGS=[$(MFLAGS)]/[$(value MFLAGS)]/$(origin MFLAGS)/$(flavor MFLAGS) MAKEFLAGS=[$(MAKEFLAGS)])
all: ; @$(info recipe MFLAGS=[$(MFLAGS)]/[$(value MFLAGS)]/$(origin MFLAGS)/$(flavor MFLAGS) MAKEFLAGS=[$(MAKEFLAGS)])true
