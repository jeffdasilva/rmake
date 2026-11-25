.SECONDEXPANSION:
colon := :
VALUE := global
all: VALUE := inherited
parent: private VALUE := parent
SET = $(shell exit 7)
REOPEN = $(eval parent$(colon) OBSERVED := $$(info reopened=[$$@|$$(VALUE)|$$(.SHELLSTATUS)]))activated
%.mid: X :::= $(REOPEN)
all: parent
parent: $$(SET) item.mid ; @printf '%s\n' 'parent=[$@|$(VALUE)|$(.SHELLSTATUS)]'
item.mid: $$(info activating=[$$@]) ; @printf '%s\n' 'child=[$@|$(VALUE)|$(.SHELLSTATUS)|$(X)]'
