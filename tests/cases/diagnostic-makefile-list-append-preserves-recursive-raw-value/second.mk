all:
	@printf 'raw=<%s> flavor=<%s> origin=<%s>\n' '$(value MAKEFILE_LIST)' '$(flavor MAKEFILE_LIST)' '$(origin MAKEFILE_LIST)'
