override MFLAGS=legacy
override .INCLUDE_DIRS=prior
override MAKEFLAGS=rR --no-print-directory
$(eval MAKEFLAGS+=-e)$(info state=[$(MAKEFLAGS)]/[$(origin MAKEFLAGS)]/[$(MFLAGS)]/[$(origin MFLAGS)]/[$(filter prior inc,$(.INCLUDE_DIRS))]/[$(origin .INCLUDE_DIRS)])
all: ; @true
