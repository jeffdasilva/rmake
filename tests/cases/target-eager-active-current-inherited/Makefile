V := global
all: V := inherited
all: child
child:;@$(eval child: RESULT := $$(V)|$$@)$(info result=[$(RESULT)]):
