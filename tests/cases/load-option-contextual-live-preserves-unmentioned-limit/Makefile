$(info read: flags=[$(subst $(HOME),fixture-home,$(MAKEFLAGS))] mflags=[$(subst $(HOME),fixture-home,$(MFLAGS))] origins=$(origin MAKEFLAGS)/$(origin MFLAGS) goals=[$(MAKECMDGOALS)])
MAKEFLAGS := -I~/inc -s
$(info live: flags=[$(subst $(HOME),fixture-home,$(MAKEFLAGS))] mflags=[$(subst $(HOME),fixture-home,$(MFLAGS))] origins=$(origin MAKEFLAGS)/$(origin MFLAGS) goals=[$(MAKECMDGOALS)])
.SECONDEXPANSION:
LATE_REPORT = $(info final: flags=[$(subst $(HOME),fixture-home,$(MAKEFLAGS))] mflags=[$(subst $(HOME),fixture-home,$(MFLAGS))] origins=$(origin MAKEFLAGS)/$(origin MFLAGS) goals=[$(MAKECMDGOALS)])
all goal INF nan +2: $$(LATE_REPORT)
.PHONY: all goal INF nan +2
