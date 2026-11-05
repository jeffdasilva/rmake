V := global
all: V := inherited
all: child
child: RESULT := base
child:;@$(eval child: RESULT += $$(V)|$$@)$(info result=[$(RESULT)] flavor=[$(flavor RESULT)]):
