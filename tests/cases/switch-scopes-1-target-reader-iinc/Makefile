.POSIX:
all: MFLAGS=legacy
all: .INCLUDE_DIRS=prior
all: MAKEFLAGS=rR --no-print-directory
$(eval MAKEFLAGS+=-Iinc)$(info state=[$(MAKEFLAGS)]/[$(origin MAKEFLAGS)]/[$(MFLAGS)]/[$(origin MFLAGS)]/[$(filter prior inc,$(.INCLUDE_DIRS))]/[$(origin .INCLUDE_DIRS)])
all: ; @true
