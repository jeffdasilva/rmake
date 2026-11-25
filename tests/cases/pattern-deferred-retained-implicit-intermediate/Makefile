.SECONDEXPANSION:
colon := :
VALUE := global
all: VALUE := inherited
parent: private VALUE := parent
SET = $(shell exit 7)
REOPEN = $(eval parent$(colon) OBSERVED := $$(info reopened=[$$@|$$(VALUE)|$$(.SHELLSTATUS)]))activated
%.mid: X :::= $(REOPEN)
all: parent
parent: $$(SET) item.out ; @printf '%s\n' 'parent=[$@|$(VALUE)|$(.SHELLSTATUS)]'
%.out: %.mid ; @:
%.mid: $$(info activating=[$$@]) %.in ; @printf '%s\n' 'child=[$@|$(VALUE)|$(.SHELLSTATUS)|$(X)]'
