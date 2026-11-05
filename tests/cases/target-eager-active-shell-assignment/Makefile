V := global
all: V := inherited
all: child
child:;@$(shell /bin/sh -c 'exit 1')$(eval child: RESULT != printf '%s|%s' '$$(V)' '$$@'; exit 3)$(info result=[$(RESULT)] flavor=[$(flavor RESULT)] status=[$(.SHELLSTATUS)]):
