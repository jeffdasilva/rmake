all: first second;@:
first: OTHER := scoped
first:;@$(foreach LEX,local,$(eval export OTHER)$(info inside=$(shell printf "[%s]" "$$OTHER")))printf "recipe=[%s]\n" "$$OTHER"
second:;@$(info next=$(origin OTHER)|$(shell printf "[%s]" "$$OTHER")):
