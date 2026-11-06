V := global
all: V := inherited
all: child
child: OWN := local
child:;@$(eval child: RESULT := $$(V)|$$@|$$(OWN))$(info result=[$(RESULT)]):
