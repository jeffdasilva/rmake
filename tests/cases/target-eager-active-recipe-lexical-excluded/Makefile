V := global
all: V := inherited
all: child
child:;@$(foreach V,lexical,$(eval child: RESULT := $$(V)|$$@))$(info result=[$(RESULT)]):
