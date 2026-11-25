.SECONDEXPANSION:
colon := :
VALUE := global
all: VALUE := inherited
parent: private VALUE := parent
SET = $(shell exit 7)
REOPEN = $(eval parent$(colon) OBSERVED := $$(info reopened=[$$@|$$(VALUE)|$$(.SHELLSTATUS)]))activated
%.mid: X :::= $(REOPEN)
all: item.mid parent
item.mid: $$(info activating=[$$@]) ; @printf '%s\n' 'child=[$@|$(VALUE)|$(.SHELLSTATUS)|$(X)]'
parent: $$(SET) ; @printf '%s\n' 'parent=[$@|$(VALUE)|$(.SHELLSTATUS)]'
