all: CAPTURED != printf '%b' '\0012\0012\0012'
all: CALLED := $(shell printf '%b' '\0012\0012\0012')
all: ; @$(info assign=[$(value CAPTURED)] func=[$(value CALLED)] status=[$(.SHELLSTATUS)])
