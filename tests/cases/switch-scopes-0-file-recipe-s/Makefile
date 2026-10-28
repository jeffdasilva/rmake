MFLAGS=legacy
.INCLUDE_DIRS=prior
MAKEFLAGS=rR --no-print-directory
all: ; @$(eval MAKEFLAGS+=-S)$(info state=[$(MAKEFLAGS)]/[$(origin MAKEFLAGS)]/[$(MFLAGS)]/[$(origin MFLAGS)]/[$(filter prior inc,$(.INCLUDE_DIRS))]/[$(origin .INCLUDE_DIRS)])true
