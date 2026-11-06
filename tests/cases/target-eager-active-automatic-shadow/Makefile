V := global
all: V := inherited
all: child
child: @ := shadow
child:;@$(eval child: RESULT := $$@)$(info result=[$(RESULT)]):
