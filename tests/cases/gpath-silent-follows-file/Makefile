seed := $(shell touch -d @1600000000 src/foo.out input; touch -d @1600000002 input)
VPATH=src
GPATH=src
.SILENT: foo.out
all: foo.out
foo.out: input
	printf "%s\n" "$@"
