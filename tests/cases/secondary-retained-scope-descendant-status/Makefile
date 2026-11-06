.SECONDEXPANSION:
all: child $$(shell /bin/sh -c 'exit 7')
all:;@$(info parent=[$@|$(.SHELLSTATUS)])$(file >>seen,parent):
child:;@$(info child=[$@|$(.SHELLSTATUS)])$(file >>seen,child):
