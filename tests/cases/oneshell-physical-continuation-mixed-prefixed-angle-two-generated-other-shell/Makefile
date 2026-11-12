.ONESHELL:
FENCE := observed
define NEXT
>>right
endef
SHELL := /bin/echo
.RECIPEPREFIX := >
all:
>@printf '<%s>\n' 'first \
$(NEXT)'
>printf '<%s>\n' 'second \
$(NEXT)'
>printf '<%s>\n' tail
