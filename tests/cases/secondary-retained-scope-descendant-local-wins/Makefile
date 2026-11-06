.SECONDEXPANSION:
all: child sibling $$(shell /bin/sh -c 'exit 7')
all:;@$(info parent=[$@|$(.SHELLSTATUS)])$(file >>seen,parent):
child: STATUS != /bin/sh -c 'exit 11'
child:;@$(info child=[$@|$(.SHELLSTATUS)])$(file >>seen,child):
sibling:;@$(info sibling=[$@|$(.SHELLSTATUS)])$(file >>seen,sibling):
