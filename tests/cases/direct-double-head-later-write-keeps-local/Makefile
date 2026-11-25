.SECONDEXPANSION:
colon := :
HEAD = $(shell exit 7)
LATER = $(shell exit 13)
WRITE = $(eval parent$(colon) TOUCH := $$(shell exit 19))
STAMP = $(eval parent$(colon) TAG := named)
READ = $(eval parent$(colon) SEEN := $$(info eager=[$$@|$$<|$$^|$$(.SHELLSTATUS)|$$(V)|$$(TAG)]))
VIEW = $(info now=[$@|$<|$^|$(.SHELLSTATUS)|$(V)|$(TAG)])
parent: V := visible
parent:: input;@$(HEAD)$(VIEW):
parent:: other;@$(LATER)$(VIEW)$(WRITE)$(READ)$(VIEW):
parent:: third;@$(VIEW):
after:;@$(READ):
