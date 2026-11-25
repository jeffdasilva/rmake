VPATH = src
GPATH = src
.SECONDEXPANSION:
colon := :
HEAD = $(shell exit 7)
NEXT = $(shell exit 13)
WRITE = $(eval $(DEST)$(colon) X := $$(shell exit 19))
VIEW = $(info read=[$@|$*|$<|$^|$(.SHELLSTATUS)|$(V)])
READ = $(eval $(DEST)$(colon) SEEN := $$(VIEW))
DEST = src/parent
CREATE = touch src/parent;
parent:: input
	@$(HEAD)$(CREATE)printf '%s\n' 'head|$@|$<|$(.SHELLSTATUS)'
parent:: other
	@$(NEXT)printf '%s\n' 'later|$@|$<|$(.SHELLSTATUS)'
after:
	@$(READ):
