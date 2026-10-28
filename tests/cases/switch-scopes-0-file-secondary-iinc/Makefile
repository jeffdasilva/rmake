MFLAGS=legacy
.INCLUDE_DIRS=prior
MAKEFLAGS=rR --no-print-directory
.SECONDEXPANSION:
all: $$(eval MAKEFLAGS+=-Iinc)$$(info state=[$$(MAKEFLAGS)]/[$$(origin MAKEFLAGS)]/[$$(MFLAGS)]/[$$(origin MFLAGS)]/[$$(filter prior inc,$$(.INCLUDE_DIRS))]/[$$(origin .INCLUDE_DIRS)])
	@true
