VPATH = src
GPATH = src
.SECONDEXPANSION:
colon := :
HEAD = $(shell exit 7)
NEXT = $(shell exit 13)
WRITE = $(eval $(DEST)$(colon) X := $$(shell exit 19))
VIEW = $(info read=[$@|$*|$<|$^|$(.SHELLSTATUS)|$(V)])
READ = $(eval $(DEST)$(colon) SEEN := $$(VIEW))
parent: V := visible
DEST = child
CREATE = touch src/parent;
parent:: input
	@$(HEAD)$(CREATE)printf '%s\n' 'head|$@|$<|$(.SHELLSTATUS)'
parent:: $$(NEXT) other child
	@printf '%s\n' 'later|$@|$<|$(.SHELLSTATUS)'
after:
	@$(READ):
child: ; @printf '%s\n' 'child|$@|$(.SHELLSTATUS)'
