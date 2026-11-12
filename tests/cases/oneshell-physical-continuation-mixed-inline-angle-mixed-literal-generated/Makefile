.ONESHELL:
FENCE := observed
define NEXT
>>right
endef
.RECIPEPREFIX := >
all: ; @printf '<%s>\n' 'first \
$(NEXT)'
>printf '<%s>\n' 'second \
>right'
>printf '<%s>\n' tail
