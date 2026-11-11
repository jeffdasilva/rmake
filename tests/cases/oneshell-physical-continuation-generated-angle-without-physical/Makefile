.ONESHELL:
FENCE := observed
define NEXT
'first'
>printf '<%s>\n' 'second'
endef
.RECIPEPREFIX := >
all:
>@printf '<%s>\n' $(NEXT)
>printf '<%s>\n' tail
