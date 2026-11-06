V := global
all: V := inherited
all: child
child:;@$(eval other: RESULT := $$(V)|$$@):
other:;@$(info result=[$(RESULT)]):
