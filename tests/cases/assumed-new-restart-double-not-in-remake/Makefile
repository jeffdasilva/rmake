$(shell if test ! -e seeded; then touch -d @1600000002 cached; touch -d @1600000001 primer; : >seeded; fi)
$(info read=$(MAKE_RESTARTS))
.DEFAULT_GOAL := all
cached:: primer
	@printf "cached\n"
all: cached
