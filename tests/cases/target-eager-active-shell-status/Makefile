V := global
all: V := inherited
all: child
child:;@$(shell /bin/sh -c 'exit 1')$(eval child: RESULT := $$(shell /bin/sh -c 'exit 7')$$(.SHELLSTATUS)|$$(V)|$$@)$(info result=[$(RESULT)] status=[$(.SHELLSTATUS)]):
