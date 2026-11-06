V := global
all: V := inherited
all: child
child:;@$(eval child: RESULT := $$(eval child: NESTED := $$(V))$$(V)|$$(NESTED)|$$@)$(info nested=[$(NESTED)] result=[$(RESULT)]):
