$(info read: flags=[$(MAKEFLAGS)] mflags=[$(MFLAGS)] origins=$(origin MAKEFLAGS)/$(origin MFLAGS) goals=[$(MAKECMDGOALS)])
MAKEFLAGS := -s
$(info live: flags=[$(MAKEFLAGS)] mflags=[$(MFLAGS)] origins=$(origin MAKEFLAGS)/$(origin MFLAGS) goals=[$(MAKECMDGOALS)])
.SECONDEXPANSION:
LATE_REPORT = $(info final: flags=[$(MAKEFLAGS)] mflags=[$(MFLAGS)] origins=$(origin MAKEFLAGS)/$(origin MFLAGS) goals=[$(MAKECMDGOALS)])
all goal INF nan +2: $$(LATE_REPORT)
.PHONY: all goal INF nan +2
