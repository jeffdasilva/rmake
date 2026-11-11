.ONESHELL:
FENCE := observed
define NEXT
'first'
>>printf '<%s>\n' 'second'
endef
SHELL := /bin/echo
.RECIPEPREFIX := >
all:
>@printf '<%s>\n' $(NEXT)
>printf '<%s>\n' tail
