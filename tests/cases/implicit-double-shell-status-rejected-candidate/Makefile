$(shell touch -d @1600000000 other; touch -d @1600000001 leaf; touch -d @1600000003 fixed input extra)
.SECONDEXPANSION:
all: leaf
	@cat expansion effects
leaf:: fixed $$(shell /bin/sh -c "exit 11") $$(file >>expansion,head[$$(.SHELLSTATUS)])
	@$(shell /bin/sh -c "exit 17")printf "head[%s]\n" "$(.SHELLSTATUS)" >> effects
leaf:: other $$(file >>expansion,tail[$$(.SHELLSTATUS)|$$<|$$*])
le%: $$(shell /bin/sh -c "exit 7") $$(file >>expansion,search[$$(.SHELLSTATUS)|$$*]) absent
	@printf "inferred[%s][%s][%s]\n" "$<" "$*" "$(.SHELLSTATUS)" >> effects
l%: $$(file >>expansion,search[$$(.SHELLSTATUS)|$$*]) input
	@printf "inferred[%s][%s][%s]\n" "$<" "$*" "$(.SHELLSTATUS)" >> effects
