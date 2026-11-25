GPATH = src
export .SHELLSTATUS
colon := :
SEARCH = $(eval vpath % src)
SRC_STATUS = $(shell exit 7)
VIEW = $(info scope=[$@|$*|$<|$^|$(.SHELLSTATUS)|$(origin .SHELLSTATUS)|$(flavor .SHELLSTATUS)|$(value .SHELLSTATUS)|$(V)])
READ = $(eval src/parent$(colon) O := $$(VIEW))
CAPTURE = $(eval src/parent$(colon) E := $$(shell /usr/bin/python3 exports.py))
REPORT = $(eval src/parent$(colon) R := $$(info env=[$$(E)]))
parent: V := source
src/parent: V := destination
src/parent: dst.in
	@$(DEST_STATUS)printf '%s\n' 'completed|$@|$<|$(.SHELLSTATUS)'
parent: src.in
	@$(SEARCH)$(SRC_STATUS)$(SRC_REPLACE)touch src/parent
after:
	@$(MUTATE)$(READ)$(CAPTURE)$(REPORT):
SRC_REPLACE = $(eval parent$(colon) unexport .SHELLSTATUS ?= ignored)
