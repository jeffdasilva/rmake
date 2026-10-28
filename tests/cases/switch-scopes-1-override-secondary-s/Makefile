.POSIX:
override MFLAGS=legacy
override .INCLUDE_DIRS=prior
override MAKEFLAGS=rR --no-print-directory
.SECONDEXPANSION:
all: $$(eval MAKEFLAGS+=-S)$$(info state=[$$(MAKEFLAGS)]/[$$(origin MAKEFLAGS)]/[$$(MFLAGS)]/[$$(origin MFLAGS)]/[$$(filter prior inc,$$(.INCLUDE_DIRS))]/[$$(origin .INCLUDE_DIRS)])
	@true
